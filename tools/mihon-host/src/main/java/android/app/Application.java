package android.app;

import android.content.Context;
import android.content.SharedPreferences;

import java.io.File;

import raygun.mihon.host.FilePreferences;

/**
 * The application extensions reach through Injekt: its preferences live in one JSON file per
 * name in the extension's data folder, which is all the host gives it.
 */
public class Application extends Context {
    private final File dir;

    public Application(File dir) {
        this.dir = dir;
    }

    @Override
    public SharedPreferences getSharedPreferences(String name, int mode) {
        return FilePreferences.open(new File(dir, "prefs_" + name.replaceAll("[^A-Za-z0-9_.-]", "_") + ".json"));
    }

    @Override
    public File getCacheDir() {
        return new File(dir, "cache");
    }

    @Override
    public File getFilesDir() {
        return dir;
    }
}
