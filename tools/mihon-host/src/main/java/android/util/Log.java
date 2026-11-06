package android.util;

/** Android's log: lines go to the host's standard error, which Raygun keeps in its log. */
public final class Log {
    private Log() {}

    private static int write(String level, String tag, String message) {
        System.err.println(level + "/" + tag + ": " + message);
        return 0;
    }

    public static int v(String tag, String message) {
        return write("V", tag, message);
    }

    public static int d(String tag, String message) {
        return write("D", tag, message);
    }

    public static int i(String tag, String message) {
        return write("I", tag, message);
    }

    public static int w(String tag, String message) {
        return write("W", tag, message);
    }

    public static int w(String tag, String message, Throwable error) {
        return write("W", tag, message + " " + error);
    }

    public static int e(String tag, String message) {
        return write("E", tag, message);
    }

    public static int e(String tag, String message, Throwable error) {
        return write("E", tag, message + " " + error);
    }
}
