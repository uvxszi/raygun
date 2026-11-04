# OPDS catalogues

Raygun reads OPDS 1.x catalogues (the Atom feeds of specs.opds.io), which comic and book servers
serve: Komga, Kavita, Calibre's content server, Calibre-Web, Ubooquity, and public libraries such
as Project Gutenberg. One catalogue is two sources: comics streamed page by page, and EPUB books
read chapter by chapter.

## Adding a catalogue

1. Browse → Sources → Servers → Add server, and choose OPDS.
2. Type the catalogue's address. A Komga or Calibre-Web server's own address is enough: Raygun
   tries the usual catalogue paths (`/opds/v1.2/catalog`, `/opds`). For Kavita, copy the OPDS
   address from its settings (it carries the user's API key).
3. A user name and password, when the catalogue asks for one (HTTP Basic).

## What it brings

| Raygun | From the catalogue |
|---|---|
| Browse → Sources → Comics and Books | The root's sections: the one for popular titles (`http://opds-spec.org/sort/popular`, else one named "All series" or similar) and the one for new ones (`sort/new`, "Latest", "Recently added"). Every section of the root is the **Section** filter. |
| Search | The catalogue's OpenSearch template. |
| Titles | A section entry (a series, a folder) is a title whose publications are its chapters; a publication listed on its own is a title of one chapter. |
| Comics | Publications with a page stream (OPDS-PSE 1.1: `pse:count` and `{pageNumber}`), read like any comic. |
| Books | Publications with an EPUB file. The file is fetched once and kept on the device (the newest 40); its chapters are its spine's documents in reading order, named by its table of contents. A series of several books lists each book's chapters, named after the book. |

Covers and pages are fetched through Raygun's image cache, signed in when the catalogue needs it.
Links to other public sites (a catalogue's mirrors, as Project Gutenberg's search) are followed
without the sign-in.

## Limits

- OPDS 2 (JSON) catalogues aren't read; the servers that serve OPDS 2 serve OPDS 1.2 as well.
- Books show their text: pictures inside an EPUB are left out.
- Read progress isn't sent back (OPDS-PSE's `lastRead` is read only by the server's own apps).
- A series with only EPUB books has no chapters under Comics, and one with only comics none under
  Books.
- PDFs and comic files without a page stream aren't readable from the catalogue.

## Checking

`komga_live` reads Komga's public demo (`demo.komga.org`, `demo@komga.org`/`komga-demo`), and
`gutenberg_opds_live` reads a book from Project Gutenberg's catalogue:

```sh
cargo test -p raygun-ext-host -- --ignored komga_live gutenberg_opds_live --nocapture
```
