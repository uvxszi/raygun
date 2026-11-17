# Draws the screenshot fixtures the text readers are measured on (`screenshot_accuracy`):
# song lists laid out as music apps show them, with the rows each should read as in
# `expected.json`. Run with `uv run --with pillow make.py <fonts folder>` (Noto Sans and Yu Gothic
# from a Windows fonts folder); the pictures are committed, so this runs only to change them.

import json
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

FONTS = Path(sys.argv[1] if len(sys.argv) > 1 else "C:/Windows/Fonts")
HERE = Path(__file__).parent

SONGS = [
    ("Bohemian Rhapsody", "Queen"),
    ("Smells Like Teen Spirit", "Nirvana"),
    ("Billie Jean", "Michael Jackson"),
    ("Hotel California", "Eagles"),
    ("Rolling in the Deep", "Adele"),
    ("Seven Nation Army", "The White Stripes"),
    ("Take On Me", "a-ha"),
    ("Africa", "Toto"),
    ("Wonderwall", "Oasis"),
    ("Dancing Queen", "ABBA"),
]
MORE = [
    ("Blinding Lights", "The Weeknd"),
    ("Mr. Brightside", "The Killers"),
    ("Clocks", "Coldplay"),
    ("Superstition", "Stevie Wonder"),
    ("Hey Ya!", "OutKast"),
    ("Creep", "Radiohead"),
    ("Valerie", "Amy Winehouse"),
    ("Juice", "Lizzo"),
]
CJK = [
    ("夜に駆ける", "YOASOBI"),
    ("Lemon", "米津玄師"),
    ("紅蓮華", "LiSA"),
    ("Pretender", "Official髭男dism"),
    ("残酷な天使のテーゼ", "高橋洋子"),
    ("丸の内サディスティック", "椎名林檎"),
]


def font(name, size):
    return ImageFont.truetype(str(FONTS / name), size)


def phone_list(path, songs, dark, title_font, artist_font, header):
    width, row = 1080, 168
    height = 360 + row * len(songs)
    background = (18, 18, 18) if dark else (255, 255, 255)
    title_colour = (255, 255, 255) if dark else (20, 20, 20)
    artist_colour = (179, 179, 179) if dark else (110, 110, 110)
    picture = Image.new("RGB", (width, height), background)
    draw = ImageDraw.Draw(picture)
    draw.text((48, 80), "9:41", fill=title_colour, font=font("NotoSans-Bold.ttf", 34))
    draw.text((48, 180), header, fill=title_colour, font=font("NotoSans-Bold.ttf", 64))
    for at, (title, artist) in enumerate(songs):
        top = 360 + at * row
        draw.rectangle((48, top + 20, 176, top + 148), fill=(90, 70, 140) if at % 2 else (60, 120, 110))
        draw.text((212, top + 30), title, fill=title_colour, font=title_font)
        draw.text((212, top + 92), artist, fill=artist_colour, font=artist_font)
        for dot in range(3):
            middle = top + 64 + dot * 18
            draw.ellipse((1012, middle, 1020, middle + 8), fill=artist_colour)
    picture.save(path, optimize=True)


def desktop_table(path, songs):
    width, row = 1600, 112
    height = 220 + row * len(songs)
    picture = Image.new("RGB", (width, height), (24, 24, 24))
    draw = ImageDraw.Draw(picture)
    head = font("NotoSans-Regular.ttf", 26)
    grey = (167, 167, 167)
    for x, text in [(40, "#"), (130, "Title"), (900, "Album"), (1440, "Length")]:
        draw.text((x, 140), text, fill=grey, font=head)
    title_font = font("NotoSans-Regular.ttf", 32)
    small = font("NotoSans-Regular.ttf", 26)
    for at, (title, artist) in enumerate(songs):
        top = 210 + at * row
        draw.text((40, top + 34), str(at + 1), fill=grey, font=small)
        draw.text((130, top + 14), title, fill=(255, 255, 255), font=title_font)
        draw.text((130, top + 60), artist, fill=grey, font=small)
        draw.text((900, top + 34), f"Greatest Hits Vol. {at % 3 + 1}", fill=grey, font=small)
        draw.text((1440, top + 34), f"{3 + at % 3}:{(at * 7) % 60:02d}", fill=grey, font=small)
    picture.save(path, optimize=True)


def main():
    regular = font("NotoSans-Regular.ttf", 44)
    small = font("NotoSans-Regular.ttf", 36)
    phone_list(HERE / "light_list.png", SONGS, False, regular, small, "Road trip")
    phone_list(HERE / "dark_list.png", MORE, True, font("NotoSans-Bold.ttf", 44), small, "Liked Songs")
    desktop_table(HERE / "desktop_table.png", SONGS[:8])
    phone_list(
        HERE / "cjk_list.png",
        CJK,
        True,
        font("YuGothM.ttc", 44),
        font("YuGothM.ttc", 36),
        "J-Pop",
    )
    expected = {
        "light_list.png": SONGS,
        "dark_list.png": MORE,
        "desktop_table.png": SONGS[:8],
        "cjk_list.png": CJK,
    }
    rows = {name: [{"title": t, "artist": a} for t, a in songs] for name, songs in expected.items()}
    (HERE / "expected.json").write_text(json.dumps(rows, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


main()
