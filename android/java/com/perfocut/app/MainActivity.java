package com.perfocut.app;

import android.app.NativeActivity;
import android.content.ContentValues;
import android.content.Intent;
import android.content.pm.ActivityInfo;
import android.database.Cursor;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.Environment;
import android.os.VibrationEffect;
import android.os.Vibrator;
import android.provider.MediaStore;
import android.provider.OpenableColumns;
import android.view.View;
import android.view.Window;
import android.widget.Toast;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.Locale;

/**
 * Окно программы рисует Rust (Slint). Здесь только то, что доступно лишь из Java:
 * выбор и сохранение файлов, вибрация, ссылки, цвет системных панелей.
 */
public class MainActivity extends NativeActivity {
    private static final int PICK = 41;
    private Vibrator vibrator;

    static {
        // Нужна для onFilePicked: NativeActivity загружает библиотеку без регистрации JNI-методов
        System.loadLibrary("perfocut");
    }

    private static native void onFilePicked(String name, byte[] data);

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        vibrator = (Vibrator) getSystemService(VIBRATOR_SERVICE);
    }

    public void pickFile() {
        runOnUiThread(new Runnable() { @Override public void run() {
            Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            i.addCategory(Intent.CATEGORY_OPENABLE);
            i.setType("*/*");
            try {
                startActivityForResult(i, PICK);
            } catch (Exception e) {
                Intent g = new Intent(Intent.ACTION_GET_CONTENT);
                g.addCategory(Intent.CATEGORY_OPENABLE);
                g.setType("*/*");
                try {
                    startActivityForResult(Intent.createChooser(g, null), PICK);
                } catch (Exception e2) {
                    Toast.makeText(MainActivity.this, e2.getMessage(), Toast.LENGTH_LONG).show();
                }
            }
        }});
    }

    @Override
    protected void onActivityResult(int req, int res, Intent data) {
        if (req != PICK) {
            super.onActivityResult(req, res, data);
            return;
        }
        if (res != RESULT_OK || data == null || data.getData() == null) return;
        final Uri uri = data.getData();
        new Thread(new Runnable() { @Override public void run() {
            try {
                String name = "file";
                Cursor c = getContentResolver().query(uri, new String[]{OpenableColumns.DISPLAY_NAME}, null, null, null);
                if (c != null) {
                    if (c.moveToFirst() && !c.isNull(0)) name = c.getString(0);
                    c.close();
                }
                InputStream in = getContentResolver().openInputStream(uri);
                ByteArrayOutputStream out = new ByteArrayOutputStream();
                byte[] buf = new byte[65536];
                int n;
                while ((n = in.read(buf)) > 0) out.write(buf, 0, n);
                in.close();
                onFilePicked(name, out.toByteArray());
            } catch (final Exception e) {
                runOnUiThread(new Runnable() { @Override public void run() {
                    Toast.makeText(MainActivity.this, String.valueOf(e.getMessage()), Toast.LENGTH_LONG).show();
                }});
            }
        }}).start();
    }

    public String saveFile(String name, String mime, byte[] bytes) {
        try {
            String safe = name.replaceAll("[\\\\/:*?\"<>|]", "_");
            // для .perfo тип не указываем как zip, иначе Android допишет «.zip»
            String type = (mime == null || mime.isEmpty() || safe.toLowerCase().endsWith(".perfo")) ? "application/octet-stream" : mime;
            if (Build.VERSION.SDK_INT >= 29) {
                ContentValues v = new ContentValues();
                v.put(MediaStore.MediaColumns.DISPLAY_NAME, safe);
                v.put(MediaStore.MediaColumns.MIME_TYPE, type);
                v.put(MediaStore.MediaColumns.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS + "/PerfoCut");
                Uri uri = getContentResolver().insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, v);
                if (uri == null) return "!MediaStore";
                OutputStream os = getContentResolver().openOutputStream(uri);
                os.write(bytes);
                os.close();
                return "Download/PerfoCut/" + safe;
            }
            File dir = getExternalFilesDir(Environment.DIRECTORY_DOWNLOADS);
            if (dir == null) dir = getFilesDir();
            dir.mkdirs();
            File f = new File(dir, safe);
            FileOutputStream os = new FileOutputStream(f);
            os.write(bytes);
            os.close();
            return f.getAbsolutePath();
        } catch (Exception e) {
            return "!" + e.getMessage();
        }
    }

    public void vibrate(String kind) {
        if (vibrator == null || !vibrator.hasVibrator()) return;
        boolean tick = "tick".equals(kind);
        try {
            if (Build.VERSION.SDK_INT >= 29) {
                vibrator.vibrate(VibrationEffect.createPredefined(tick ? VibrationEffect.EFFECT_TICK : VibrationEffect.EFFECT_CLICK));
            } else if (Build.VERSION.SDK_INT >= 26) {
                vibrator.vibrate(VibrationEffect.createOneShot(tick ? 8 : 18, VibrationEffect.DEFAULT_AMPLITUDE));
            } else {
                vibrator.vibrate(tick ? 8 : 18);
            }
        } catch (Exception e) {
            // вибрация недоступна
        }
    }

    public void openUrl(final String url) {
        runOnUiThread(new Runnable() { @Override public void run() {
            try {
                startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(url)));
            } catch (Exception e) {
                Toast.makeText(MainActivity.this, url, Toast.LENGTH_LONG).show();
            }
        }});
    }

    public float fontScale() {
        return getResources().getConfiguration().fontScale;
    }

    public String locale() {
        return Locale.getDefault().toLanguageTag();
    }

    public void goBack() {
        runOnUiThread(new Runnable() { @Override public void run() { moveTaskToBack(true); } });
    }

    /** Полный экран для рисования: горизонтально, без строки состояния и навигации. */
    public void fullCanvas(final boolean on) {
        runOnUiThread(new Runnable() { @Override public void run() {
            try {
                setRequestedOrientation(on
                    ? ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
                    : ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED);
            } catch (Exception e) {
                // ориентацию менять нельзя — холст всё равно развернётся
            }
            Window w = getWindow();
            View d = w.getDecorView();
            int f = d.getSystemUiVisibility();
            int hide = View.SYSTEM_UI_FLAG_FULLSCREEN | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY | View.SYSTEM_UI_FLAG_LAYOUT_STABLE;
            f = on ? (f | hide) : (f & ~hide);
            d.setSystemUiVisibility(f);
        }});
    }

    public void setBars(final int color, final boolean light) {
        runOnUiThread(new Runnable() { @Override public void run() {
            Window w = getWindow();
            w.setStatusBarColor(color);
            w.setNavigationBarColor(color);
            View d = w.getDecorView();
            int f = d.getSystemUiVisibility();
            if (Build.VERSION.SDK_INT >= 23) {
                f = light ? (f | View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR) : (f & ~View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR);
            }
            if (Build.VERSION.SDK_INT >= 26) {
                f = light ? (f | View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR) : (f & ~View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR);
            }
            d.setSystemUiVisibility(f);
        }});
    }
}
