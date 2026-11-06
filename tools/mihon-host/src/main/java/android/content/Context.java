package android.content;

import java.io.File;

/** Android's context, as much of it as extensions use: preferences and folders. */
public abstract class Context {
    public static final int MODE_PRIVATE = 0;

    public abstract SharedPreferences getSharedPreferences(String name, int mode);

    public abstract File getCacheDir();

    public abstract File getFilesDir();

    public Context getApplicationContext() {
        return this;
    }

    public String getPackageName() {
        return "app.raygun.player";
    }
}
