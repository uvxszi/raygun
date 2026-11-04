# Audiobookshelf

Raygun connects to an Audiobookshelf server as a source of audiobooks.

## Adding a server

1. Browse → Sources → Servers → Add server, and choose Audiobookshelf.
2. Type the server's address: `books.example.org`, or `192.168.1.20:13378`.
3. Sign in with a user name and password, or turn on **Use an API key** and paste a key made in
   the server's settings (Audiobookshelf 2.26 and newer).

A password sign-in gives an access token that lasts an hour; while the server is connected,
Raygun renews it in the background (with the refresh token the server gives, else by signing in
again), so chapters keep playing and downloading. The password stays in Raygun's encrypted
secrets store.

## What it brings

| Raygun | From the server |
|---|---|
| Library → Audio → Audiobooks, Browse all | The book libraries' books, newest added first. Books with only an ebook are left out. |
| Search → Audiobooks | Each book library's search. |
| Genres | The library's own genre that matches the one asked for. |
| A book's page | Its audio files in order, as chapters. |

Chapter addresses are kept without a sign-in; Raygun adds the current token when it streams or
downloads one.

## Limits

- A book that is one file with chapters inside (an M4B) is one chapter in Raygun.
- Listening progress isn't sent to the server.
- Podcast libraries aren't listed: their feeds can be followed directly.
- There's no public demo server to check against; the client is checked against a stand-in of
  the API built from its documentation and source.
