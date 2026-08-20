package com.be3.block;

import android.app.NativeActivity;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.provider.OpenableColumns;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;

public final class MainActivity extends NativeActivity {
    private static final int PICK_FILE_REQUEST = 0x8E31;
    private static final int MAX_FILE_BYTES = 128 * 1024 * 1024;
    private static final int COPY_BUFFER_BYTES = 64 * 1024;
    private static MainActivity current;

    static { System.loadLibrary("block_app_lib"); }

    @Override
    protected void onCreate(Bundle state) {
        current = this;
        super.onCreate(state);
    }

    @Override
    protected void onDestroy() {
        if (current == this) current = null;
        super.onDestroy();
    }

    public static boolean pickFile(String mimeTypes) {
        MainActivity activity = current;
        if (activity == null) return false;
        activity.runOnUiThread(() -> activity.launchPicker(mimeTypes));
        return true;
    }

    private void launchPicker(String mimeTypes) {
        String[] types = mimeTypes.split(",");
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.setType(types.length == 1 ? types[0] : "*/*");
        if (types.length > 1) intent.putExtra(Intent.EXTRA_MIME_TYPES, types);
        try {
            startActivityForResult(intent, PICK_FILE_REQUEST);
        } catch (Exception error) {
            nativeFilePicked(null, null, "No app is available to choose a file");
        }
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data) {
        if (request != PICK_FILE_REQUEST) {
            super.onActivityResult(request, result, data);
            return;
        }
        Uri uri = result == RESULT_OK && data != null ? data.getData() : null;
        if (uri == null) {
            nativeFilePicked(null, null, null);
            return;
        }
        new Thread(() -> {
            try {
                nativeFilePicked(displayName(uri), read(uri), null);
            } catch (Throwable error) {
                nativeFilePicked(null, null, "Could not read the chosen file: " + error);
            }
        }, "block-app-file-picker").start();
    }

    private String displayName(Uri uri) {
        String[] columns = { OpenableColumns.DISPLAY_NAME };
        try (Cursor cursor = getContentResolver().query(uri, columns, null, null, null)) {
            if (cursor != null && cursor.moveToFirst()) {
                String name = cursor.getString(0);
                if (name != null && !name.isEmpty()) return name;
            }
        } catch (Exception ignored) {}
        String path = uri.getLastPathSegment();
        return path == null ? "" : path.substring(path.lastIndexOf('/') + 1);
    }

    private byte[] read(Uri uri) throws Exception {
        try (InputStream stream = getContentResolver().openInputStream(uri)) {
            if (stream == null) throw new IllegalStateException("it could not be opened");
            ByteArrayOutputStream bytes = new ByteArrayOutputStream();
            byte[] buffer = new byte[COPY_BUFFER_BYTES];
            int read;
            while ((read = stream.read(buffer)) != -1) {
                if (bytes.size() + read > MAX_FILE_BYTES) {
                    throw new IllegalStateException("it is larger than " + (MAX_FILE_BYTES / (1024 * 1024)) + " MB");
                }
                bytes.write(buffer, 0, read);
            }
            return bytes.toByteArray();
        }
    }

    private static native void nativeFilePicked(String name, byte[] data, String error);
}
