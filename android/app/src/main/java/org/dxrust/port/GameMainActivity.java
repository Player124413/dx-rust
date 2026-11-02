package org.dxrust.port;

import android.app.Activity;
import android.graphics.Insets;
import android.os.Build;
import android.os.Bundle;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.view.WindowManager;
import com.google.androidgamesdk.GameActivity;
import java.io.File;
import java.util.LinkedHashMap;
import java.util.Map;

public final class GameMainActivity extends GameActivity {
    static { System.loadLibrary("dxview"); }

    @Override public void onCreate(Bundle state) {
        writeSurface(this);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        super.onCreate(state);
        hideSystemUi();
    }
    @Override public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus) { hideSystemUi(); writeSurface(this); }
    }
    @Override protected void onResume() { super.onResume(); writeSurface(this); hideSystemUi(); }

    private void hideSystemUi() {
        if (Build.VERSION.SDK_INT >= 30) {
            WindowInsetsController c = getWindow().getInsetsController();
            if (c != null) { c.hide(WindowInsets.Type.systemBars()); c.setSystemBarsBehavior(WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE); }
        } else {
            getWindow().getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY | View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                    | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                    | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION | View.SYSTEM_UI_FLAG_FULLSCREEN);
        }
    }

    /** Physical surface size and cutout/system-bar insets for the Rust safe-area layout. */
    static void writeSurface(Activity a) {
        int w = a.getResources().getDisplayMetrics().widthPixels;
        int h = a.getResources().getDisplayMetrics().heightPixels;
        int l = 0, t = 0, r = 0, b = 0;
        if (Build.VERSION.SDK_INT >= 30) {
            WindowInsets i = a.getWindow().getDecorView().getRootWindowInsets();
            if (i != null) { Insets ins = i.getInsetsIgnoringVisibility(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout()); l = ins.left; t = ins.top; r = ins.right; b = ins.bottom; }
        }
        Map<String,String> m = new LinkedHashMap<>();
        m.put("ViewWidth", String.valueOf(w)); m.put("ViewHeight", String.valueOf(h));
        m.put("InsetLeft", String.valueOf(l)); m.put("InsetTop", String.valueOf(t));
        m.put("InsetRight", String.valueOf(r)); m.put("InsetBottom", String.valueOf(b));
        LauncherActivity.writeKv(new File(a.getFilesDir(), "surface.txt"), m);
    }
}
