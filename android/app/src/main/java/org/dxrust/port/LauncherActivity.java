package org.dxrust.port;

import android.app.Activity;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.provider.DocumentsContract;
import android.text.InputType;
import android.view.Gravity;
import android.view.View;
import android.widget.*;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.*;

/** Native Android launcher: no WebView, no network, no game data bundled. */
public final class LauncherActivity extends Activity {
    private static final int PICK_TREE = 41;
    private EditText pathInput;
    private Spinner quality, texture;
    private CheckBox fps, edit, touch;
    private TextView status;
    private volatile boolean cancelImport;
    private Thread worker;

    @Override public void onCreate(Bundle b) {
        super.onCreate(b);
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(dp(22), dp(16), dp(22), dp(16));
        root.setBackgroundColor(0xff071014);
        ScrollView scroll = new ScrollView(this);
        scroll.addView(root);
        setContentView(scroll);

        TextView title = text("DX RUST", 30, 0xffe8fff8);
        title.setGravity(Gravity.CENTER_VERTICAL);
        root.addView(title);
        root.addView(text("Android port • adaptive 60 FPS target • editable touch controls", 14, 0xff8fb8ae));
        root.addView(text("Bring your own Deus Ex (GOTY) installation. No copyrighted game data is included.", 13, 0xffc8d6d2));

        pathInput = new EditText(this);
        pathInput.setSingleLine(true);
        pathInput.setInputType(InputType.TYPE_CLASS_TEXT);
        pathInput.setTextColor(0xffe8fff8);
        pathInput.setHintTextColor(0xff6f8f87);
        pathInput.setHint("Game folder containing System/DeusEx.u");
        pathInput.setBackgroundColor(0xff101c22);
        pathInput.setPadding(dp(10), dp(10), dp(10), dp(10));
        root.addView(pathInput, new LinearLayout.LayoutParams(-1, -2));

        LinearLayout row = row();
        Button imp = button("IMPORT GAME FOLDER");
        imp.setOnClickListener(v -> pickFolder());
        Button check = button("CHECK");
        check.setOnClickListener(v -> validate(true));
        row.addView(imp); row.addView(check); root.addView(row);

        quality = spinner(new String[]{"Auto — recommended", "Battery — lowest load", "Balanced", "High — experimental effects"});
        texture = spinner(new String[]{"Textures: Auto (ETC2 when supported)", "Textures: prefer ETC2", "Textures: RGBA compatibility"});
        root.addView(labelled("Graphics preset", quality));
        root.addView(labelled("Texture path", texture));
        fps = check("Show FPS / render-scale overlay", false);
        edit = check("Open touch-control editor on start", false);
        touch = check("Enable touch controls", true);
        root.addView(fps); root.addView(edit); root.addView(touch);

        Button start = button("PLAY");
        start.setTextSize(20);
        start.setOnClickListener(v -> startGame());
        root.addView(start, new LinearLayout.LayoutParams(-1, dp(56)));
        status = text("", 14, 0xff9fe8d5);
        root.addView(status);
        root.addView(text("Import copies System/Maps/Textures/Sounds/Music into this app's private external folder. " +
                "You can also type a direct path if your device exposes one. Saves and settings stay in app storage.", 12, 0xff7f9a93));
        load();
    }

    private TextView text(String s, int sp, int color) { TextView t = new TextView(this); t.setText(s); t.setTextSize(sp); t.setTextColor(color); t.setPadding(0, dp(5), 0, dp(5)); return t; }
    private Button button(String s) { Button b = new Button(this); b.setAllCaps(false); b.setText(s); b.setTextColor(0xff071014); b.setBackgroundColor(0xff4de6c4); return b; }
    private CheckBox check(String s, boolean on) { CheckBox c = new CheckBox(this); c.setText(s); c.setTextColor(0xffdcefe9); c.setChecked(on); return c; }
    private Spinner spinner(String[] items) { Spinner s = new Spinner(this); ArrayAdapter<String> a = new ArrayAdapter<>(this, android.R.layout.simple_spinner_dropdown_item, items); s.setAdapter(a); s.setBackgroundColor(0xff101c22); return s; }
    private LinearLayout row() { LinearLayout r = new LinearLayout(this); r.setOrientation(LinearLayout.HORIZONTAL); r.setGravity(Gravity.CENTER_VERTICAL); return r; }
    private LinearLayout labelled(String label, View v) { LinearLayout r = row(); TextView t = text(label, 14, 0xffc8d6d2); t.setWidth(dp(150)); r.addView(t); r.addView(v, new LinearLayout.LayoutParams(0, -2, 1)); return r; }
    private int dp(int v) { return Math.round(getResources().getDisplayMetrics().density * v); }

    private File importDir() { File base = getExternalFilesDir(null); if (base == null) base = getFilesDir(); return new File(base, "DeusEx"); }
    private void load() {
        Map<String,String> l = readKv(new File(getFilesDir(), "launcher.txt"));
        Map<String,String> s = readKv(new File(getFilesDir(), "settings.txt"));
        String p = l.getOrDefault("GamePath", importDir().getAbsolutePath());
        pathInput.setText(p);
        quality.setSelection(parseInt(s.get("MobileQuality"), 0, 0, 3));
        texture.setSelection(parseInt(s.get("MobileTextureMode"), 0, 0, 2));
        fps.setChecked(parseFloat(s.get("MobileFps")) > 0.5f);
        touch.setChecked(parseFloat(s.getOrDefault("TouchEnabled", "1")) > 0.5f);
        if (isGameDir(new File(p))) status.setText("Ready: " + p);
    }
    private static int parseInt(String s, int d, int lo, int hi) { try { return Math.max(lo, Math.min(hi, Integer.parseInt(s.trim()))); } catch (Exception e) { return d; } }
    private static float parseFloat(String s) { try { return Float.parseFloat(s.trim()); } catch (Exception e) { return 0; } }

    private void pickFolder() {
        Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
        i.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
        try { startActivityForResult(i, PICK_TREE); } catch (Exception e) { status.setText("Folder picker unavailable: " + e.getMessage()); }
    }
    @Override protected void onActivityResult(int req, int res, Intent data) {
        super.onActivityResult(req, res, data);
        if (req != PICK_TREE || res != RESULT_OK || data == null || data.getData() == null) return;
        Uri tree = data.getData();
        try { getContentResolver().takePersistableUriPermission(tree, Intent.FLAG_GRANT_READ_URI_PERMISSION); } catch (Exception ignored) {}
        cancelImport = false;
        File dest = importDir();
        status.setText("Importing to " + dest.getAbsolutePath() + " …");
        worker = new Thread(() -> {
            try {
                String rootId = DocumentsContract.getTreeDocumentId(tree);
                long[] count = {0, 0};
                for (String dir : new String[]{"System", "Maps", "Textures", "Sounds", "Music"}) {
                    String id = findChild(tree, rootId, dir);
                    if (id != null) copyDir(tree, id, new File(dest, dir), count);
                }
                runOnUiThread(() -> {
                    if (cancelImport) { status.setText("Import cancelled."); return; }
                    pathInput.setText(dest.getAbsolutePath());
                    validate(true);
                    status.setText("Imported " + count[0] + " files (" + (count[1] / (1024 * 1024)) + " MiB). " + (isGameDir(dest) ? "Ready." : "System/DeusEx.u was not found."));
                });
            } catch (Exception e) {
                runOnUiThread(() -> status.setText("Import failed: " + e.getMessage()));
            }
        }, "dx-import");
        worker.start();
    }
    private String findChild(Uri tree, String parent, String name) throws Exception {
        Uri uri = DocumentsContract.buildChildDocumentsUriUsingTree(tree, parent);
        try (Cursor c = getContentResolver().query(uri, new String[]{DocumentsContract.Document.COLUMN_DOCUMENT_ID, DocumentsContract.Document.COLUMN_DISPLAY_NAME}, null, null, null)) {
            if (c == null) return null;
            while (c.moveToNext()) if (name.equalsIgnoreCase(c.getString(1))) return c.getString(0);
        }
        return null;
    }
    private void copyDir(Uri tree, String docId, File dest, long[] count) throws Exception {
        if (cancelImport) return;
        if (!dest.exists() && !dest.mkdirs()) throw new IOException("Cannot create " + dest);
        Uri uri = DocumentsContract.buildChildDocumentsUriUsingTree(tree, docId);
        List<String[]> children = new ArrayList<>();
        try (Cursor c = getContentResolver().query(uri, new String[]{DocumentsContract.Document.COLUMN_DOCUMENT_ID, DocumentsContract.Document.COLUMN_DISPLAY_NAME, DocumentsContract.Document.COLUMN_MIME_TYPE, DocumentsContract.Document.COLUMN_SIZE}, null, null, null)) {
            if (c == null) throw new IOException("Cannot list " + dest.getName());
            while (c.moveToNext()) children.add(new String[]{c.getString(0), c.getString(1), c.getString(2), c.isNull(3) ? "-1" : c.getString(3)});
        }
        for (String[] ch : children) {
            if (cancelImport) return;
            String name = ch[1]; if (name == null || name.contains("/") || name.contains("\\") || name.equals(".") || name.equals("..")) continue;
            File out = new File(dest, name);
            if (DocumentsContract.Document.MIME_TYPE_DIR.equals(ch[2])) copyDir(tree, ch[0], out, count);
            else {
                long size = Long.parseLong(ch[3]);
                if (out.exists() && size >= 0 && out.length() == size) continue;
                try (InputStream in = getContentResolver().openInputStream(DocumentsContract.buildDocumentUriUsingTree(tree, ch[0])); OutputStream os = new FileOutputStream(out)) {
                    if (in == null) throw new IOException("Cannot open " + name);
                    byte[] buf = new byte[1 << 20]; int n;
                    while ((n = in.read(buf)) > 0) { if (cancelImport) return; os.write(buf, 0, n); count[1] += n; }
                }
                count[0]++;
                if (count[0] % 32 == 0) runOnUiThread(() -> status.setText("Imported " + count[0] + " files…"));
            }
        }
    }

    private boolean validate(boolean loud) {
        File f = new File(pathInput.getText().toString().trim());
        boolean ok = isGameDir(f);
        if (loud) status.setText(ok ? "Game data found. Ready to play." : "Not a Deus Ex folder: need System/DeusEx.u and Maps.");
        return ok;
    }
    static boolean isGameDir(File f) {
        File sys = new File(f, "System");
        boolean u = false; File[] list = sys.listFiles(); if (list != null) for (File x : list) if (x.getName().equalsIgnoreCase("DeusEx.u")) u = true;
        return u && new File(f, "Maps").isDirectory();
    }
    private void startGame() {
        if (worker != null && worker.isAlive()) { status.setText("Wait for the import to finish first."); return; }
        if (!validate(true)) return;
        Map<String,String> l = readKv(new File(getFilesDir(), "launcher.txt"));
        l.put("GamePath", pathInput.getText().toString().trim());
        writeKv(new File(getFilesDir(), "launcher.txt"), l);
        Map<String,String> s = readKv(new File(getFilesDir(), "settings.txt"));
        s.put("MobileQuality", String.valueOf(quality.getSelectedItemPosition()));
        s.put("MobileTextureMode", String.valueOf(texture.getSelectedItemPosition()));
        s.put("MobileFps", fps.isChecked() ? "1" : "0");
        s.put("TouchEnabled", touch.isChecked() ? "1" : "0");
        s.put("TouchEditOnStart", edit.isChecked() ? "1" : "0");
        s.put("DisplayMode", "0"); s.put("VSync", "1");
        writeKv(new File(getFilesDir(), "settings.txt"), s);
        GameMainActivity.writeSurface(this);
        startActivity(new Intent(this, GameMainActivity.class));
    }
    static Map<String,String> readKv(File f) {
        Map<String,String> m = new LinkedHashMap<>();
        try (BufferedReader r = new BufferedReader(new InputStreamReader(new FileInputStream(f), StandardCharsets.UTF_8))) {
            String line; while ((line = r.readLine()) != null) { line = line.trim(); if (line.isEmpty() || line.startsWith("#")) continue; int i = line.indexOf('='); if (i > 0) m.put(line.substring(0, i).trim(), line.substring(i + 1).trim()); }
        } catch (Exception ignored) {}
        return m;
    }
    static void writeKv(File f, Map<String,String> m) {
        File tmp = new File(f.getAbsolutePath() + ".tmp");
        try (OutputStreamWriter w = new OutputStreamWriter(new FileOutputStream(tmp), StandardCharsets.UTF_8)) {
            w.write("# DX RUST Android\n"); for (Map.Entry<String,String> e : m.entrySet()) w.write(e.getKey() + "=" + e.getValue() + "\n");
        } catch (Exception ignored) { return; }
        //noinspection ResultOfMethodCallIgnored
        tmp.renameTo(f);
    }
    @Override protected void onDestroy() { cancelImport = true; super.onDestroy(); }
}
