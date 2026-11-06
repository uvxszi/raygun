package android.os;

/** What extensions read of Android's build: a recent version. */
public final class Build {
    public static final String MANUFACTURER = "Raygun";
    public static final String MODEL = "Desktop";

    private Build() {}

    public static final class VERSION {
        public static final int SDK_INT = 35;
        public static final String RELEASE = "15";

        private VERSION() {}
    }

    public static final class VERSION_CODES {
        public static final int N = 24;
        public static final int O = 26;
        public static final int P = 28;
        public static final int Q = 29;
        public static final int R = 30;
        public static final int S = 31;
        public static final int TIRAMISU = 33;
        public static final int UPSIDE_DOWN_CAKE = 34;

        private VERSION_CODES() {}
    }
}
