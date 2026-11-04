# Other formats

Besides its own extensions, Raygun runs extensions and addons made for other apps, from
repositories and addresses people add themselves; it ships and pre-adds none of them. Their code
runs sandboxed like Raygun's own extensions, and Stremio addons are web services Raygun asks.
Each format is documented by the project that made it.

| Format                 | What it gives Raygun                                  | Add it in Raygun                                        | Made for                                                    |
| ---------------------- | ----------------------------------------------------- | ------------------------------------------------------- | ----------------------------------------------------------- |
| Shosetsu scripts (Lua) | Book sources                                          | Extension repositories: a Shosetsu repository's address | [Shosetsu](https://gitlab.com/shosetsuorg/extensions)       |
| Nuclear plugins        | Song search, streams and lyrics, converted at install | Extension repositories: Nuclear's plugin registry       | [Nuclear](https://github.com/NuclearPlayer/plugin-registry) |
| Miru extensions        | Video, comics and book sources                        | Extension repositories: a Miru repository's address     | [Miru](https://github.com/miru-project/miru-app)            |
| Stremio addons         | Films and series: catalogues, streams, subtitles      | Browse → Sources → Addons, by the addon's address       | [Stremio](https://github.com/Stremio/stremio-addon-sdk)     |

Raygun reads a Shosetsu, Nuclear or Miru repository as it is: add its address as you would one of
Raygun's own. A Raygun repository may list packages of these formats too, by their `format`
([repositories](repositories.md#indexjson)). Other formats Raygun runs are documented with their
support in Raygun.
