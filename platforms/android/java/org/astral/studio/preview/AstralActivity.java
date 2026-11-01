package org.astral.studio.preview;

import android.app.NativeActivity;
import android.content.ContentValues;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.os.Build;
import android.graphics.Insets;
import android.view.WindowInsets;
import android.provider.MediaStore;
import android.widget.Toast;
import java.io.File;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;

/** Platform file picker and share sheet. Rust owns the editable document. */
public final class AstralActivity extends NativeActivity {
    private static final int IMPORT_PROJECT = 410;
    private volatile int[] safeInsets = new int[] {0,0,0,0};

    @Override protected void onCreate(Bundle state) {
        super.onCreate(state);
        getWindow().getDecorView().setOnApplyWindowInsetsListener((view,insets) -> {
            if (Build.VERSION.SDK_INT >= 30) {
                Insets safe = insets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout() | WindowInsets.Type.ime());
                safeInsets = new int[] {safe.left,safe.top,safe.right,safe.bottom};
            } else {
                safeInsets = new int[] {insets.getSystemWindowInsetLeft(),insets.getSystemWindowInsetTop(),insets.getSystemWindowInsetRight(),insets.getSystemWindowInsetBottom()};
            }
            return insets;
        });
        getWindow().getDecorView().requestApplyInsets();
    }

    public int[] getSafeInsets() { return safeInsets.clone(); }

    public void chooseProject() {
        runOnUiThread(() -> {
            Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            intent.setType("*/*");
            intent.addCategory(Intent.CATEGORY_OPENABLE);
            startActivityForResult(intent, IMPORT_PROJECT);
        });
    }

    @Override protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request != IMPORT_PROJECT || result != RESULT_OK || data == null || data.getData() == null) return;
        Uri uri = data.getData();
        new Thread(() -> {
            File directory = new File(getFilesDir(), "projects");
            directory.mkdirs();
            File temporary = new File(directory, "import-pending.tmp");
            try (InputStream in = getContentResolver().openInputStream(uri);
                 OutputStream out = Files.newOutputStream(temporary.toPath())) {
                if (in == null) throw new IllegalStateException("Cannot open selected document");
                byte[] buffer = new byte[65536];
                long total = 0;
                int count;
                while ((count = in.read(buffer)) != -1) {
                    total += count;
                    if (total > 32L * 1024 * 1024) throw new IllegalArgumentException("Preview import limit is 32 MiB");
                    out.write(buffer, 0, count);
                }
                out.close();
                Files.move(temporary.toPath(), new File(directory, "import-incoming.astral").toPath(), StandardCopyOption.REPLACE_EXISTING);
                notifyUser("Project received. Return to Astral to review it.");
            } catch (Exception e) {
                temporary.delete();
                notifyUser("Import failed: " + e.getMessage());
            }
        }, "Astral import").start();
    }

    public void sharePreview() {
        new Thread(() -> {
            Uri uri = null;
            try {
                File file = new File(getFilesDir(), "projects/exports/frame.png");
                ContentValues values = new ContentValues();
                values.put(MediaStore.Images.Media.DISPLAY_NAME, "Astral-" + System.currentTimeMillis() + ".png");
                values.put(MediaStore.Images.Media.MIME_TYPE, "image/png");
                values.put(MediaStore.Images.Media.RELATIVE_PATH, "Pictures/Astral");
                values.put(MediaStore.Images.Media.IS_PENDING, 1);
                uri = getContentResolver().insert(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, values);
                if (uri == null) throw new IllegalStateException("Gallery unavailable");
                try (OutputStream out = getContentResolver().openOutputStream(uri)) {
                    if (out == null) throw new IllegalStateException("Gallery write unavailable");
                    Files.copy(file.toPath(), out);
                }
                values.clear();
                values.put(MediaStore.Images.Media.IS_PENDING, 0);
                getContentResolver().update(uri, values, null, null);
                final Uri shared = uri;
                runOnUiThread(() -> {
                    Intent intent = new Intent(Intent.ACTION_SEND).setType("image/png");
                    intent.putExtra(Intent.EXTRA_STREAM, shared);
                    intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
                    startActivity(Intent.createChooser(intent, "Share Astral frame"));
                });
            } catch (Exception e) {
                if (uri != null) getContentResolver().delete(uri, null, null);
                notifyUser("Export failed: " + e.getMessage());
            }
        }, "Astral PNG share").start();
    }

    private void notifyUser(String message) {
        runOnUiThread(() -> Toast.makeText(this, message, Toast.LENGTH_LONG).show());
    }
}
