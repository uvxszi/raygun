package android.net;

import java.net.URI;
import java.net.URLDecoder;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

/** Android's Uri, as far as extensions read addresses with it. */
public final class Uri {
    private final URI uri;

    private Uri(URI uri) {
        this.uri = uri;
    }

    public static Uri parse(String text) {
        return new Uri(URI.create(text.replace(" ", "%20")));
    }

    public String getScheme() {
        return uri.getScheme();
    }

    public String getHost() {
        return uri.getHost();
    }

    public String getPath() {
        return uri.getPath();
    }

    public String getQuery() {
        return uri.getQuery();
    }

    public String getFragment() {
        return uri.getFragment();
    }

    public String getLastPathSegment() {
        List<String> segments = getPathSegments();
        return segments.isEmpty() ? null : segments.get(segments.size() - 1);
    }

    public List<String> getPathSegments() {
        List<String> segments = new ArrayList<>();
        String path = uri.getPath();
        if (path == null) return segments;
        for (String part : path.split("/")) if (!part.isEmpty()) segments.add(part);
        return segments;
    }

    public String getQueryParameter(String name) {
        String query = uri.getRawQuery();
        if (query == null) return null;
        for (String pair : query.split("&")) {
            int at = pair.indexOf('=');
            String key = URLDecoder.decode(at < 0 ? pair : pair.substring(0, at), StandardCharsets.UTF_8);
            if (key.equals(name)) {
                return at < 0 ? "" : URLDecoder.decode(pair.substring(at + 1), StandardCharsets.UTF_8);
            }
        }
        return null;
    }

    @Override
    public String toString() {
        return uri.toString();
    }
}
