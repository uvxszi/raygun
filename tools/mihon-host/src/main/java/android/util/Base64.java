package android.util;

import java.nio.charset.StandardCharsets;

/** Android's Base64, over Java's: its flags pick the alphabet, padding and line breaks. */
public final class Base64 {
    public static final int DEFAULT = 0;
    public static final int NO_PADDING = 1;
    public static final int NO_WRAP = 2;
    public static final int CRLF = 4;
    public static final int URL_SAFE = 8;
    public static final int NO_CLOSE = 16;

    private Base64() {}

    public static byte[] decode(String text, int flags) {
        return decode(text.getBytes(StandardCharsets.US_ASCII), flags);
    }

    public static byte[] decode(byte[] input, int flags) {
        return decode(input, 0, input.length, flags);
    }

    public static byte[] decode(byte[] input, int offset, int length, int flags) {
        String text = new String(input, offset, length, StandardCharsets.US_ASCII).replaceAll("\\s", "");
        // Android reads either alphabet and missing padding.
        text = text.replace('-', '+').replace('_', '/');
        while (text.length() % 4 != 0) text += "=";
        try {
            return java.util.Base64.getDecoder().decode(text);
        } catch (IllegalArgumentException error) {
            throw new IllegalArgumentException("bad base-64", error);
        }
    }

    public static String encodeToString(byte[] input, int flags) {
        return new String(encode(input, flags), StandardCharsets.US_ASCII);
    }

    public static String encodeToString(byte[] input, int offset, int length, int flags) {
        byte[] part = new byte[length];
        System.arraycopy(input, offset, part, 0, length);
        return encodeToString(part, flags);
    }

    public static byte[] encode(byte[] input, int flags) {
        java.util.Base64.Encoder encoder =
                (flags & URL_SAFE) != 0 ? java.util.Base64.getUrlEncoder() : java.util.Base64.getEncoder();
        if ((flags & NO_PADDING) != 0) encoder = encoder.withoutPadding();
        String text = encoder.encodeToString(input);
        if ((flags & NO_WRAP) == 0) {
            StringBuilder wrapped = new StringBuilder();
            String end = (flags & CRLF) != 0 ? "\r\n" : "\n";
            for (int at = 0; at < text.length(); at += 76) {
                wrapped.append(text, at, Math.min(text.length(), at + 76)).append(end);
            }
            text = wrapped.toString();
        }
        return text.getBytes(StandardCharsets.US_ASCII);
    }

    public static byte[] encode(byte[] input, int offset, int length, int flags) {
        byte[] part = new byte[length];
        System.arraycopy(input, offset, part, 0, length);
        return encode(part, flags);
    }
}
