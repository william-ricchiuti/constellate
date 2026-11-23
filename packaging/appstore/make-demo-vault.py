#!/usr/bin/env python3
"""Builds the demo notes folder for App Review: a small Obsidian vault of stargazing notes.

    python3 packaging/appstore/make-demo-vault.py [out.zip]

Writes `packaging/appstore/demo-vault.zip` (default), holding one folder,
"Constellate Demo Vault". About 90 linked Markdown notes with tags: hubs, constellations,
stars, deep-sky objects, the planets, gear and a dated observing log, so the graph has
clusters, hubs and a few cross-links. The output is the same on every run (fixed random
seed, fixed file times).
"""

import random
import sys
import zipfile
from pathlib import Path

ROOT = "Constellate Demo Vault"
EPOCH = (2026, 1, 1, 0, 0, 0)

# name: (constellation, one line)
STARS = {
    "Betelgeuse": ("Orion", "A red supergiant on Orion's shoulder."),
    "Rigel": ("Orion", "A blue supergiant at Orion's foot."),
    "Polaris": ("Ursa Minor", "The North Star, almost exactly above the north pole."),
    "Dubhe": ("Ursa Major", "One of the two pointer stars of the Big Dipper."),
    "Merak": ("Ursa Major", "The other pointer star; the line through both leads to Polaris."),
    "Vega": ("Lyra", "One of the brightest stars of the summer sky."),
    "Deneb": ("Cygnus", "The tail of the swan, very far away and very luminous."),
    "Altair": ("Aquila", "A fast-spinning star in the eagle."),
    "Antares": ("Scorpius", "The red heart of the scorpion."),
    "Aldebaran": ("Taurus", "The orange eye of the bull."),
    "Castor": ("Gemini", "One of the twins; a system of six stars."),
    "Pollux": ("Gemini", "The brighter twin, an orange giant."),
    "Regulus": ("Leo", "The heart of the lion, close to the ecliptic."),
    "Mirach": ("Andromeda", "A red giant; a guide star to the Andromeda Galaxy."),
    "Algol": ("Perseus", "The demon star, an eclipsing binary that dims every few days."),
    "Sirius": ("Canis Major", "The brightest star in the night sky."),
    "Arcturus": ("Boötes", "The brightest star north of the celestial equator."),
    "Acrux": ("Crux", "The foot of the Southern Cross."),
    "Alpha Centauri": ("Centaurus", "The nearest star system to the Sun."),
    "Schedar": ("Cassiopeia", "The brightest star of the W."),
}

CONSTELLATIONS = {
    "Orion": "The hunter, the easiest winter constellation to find.",
    "Ursa Major": "The great bear; seven of its stars are the Big Dipper.",
    "Ursa Minor": "The little bear, with Polaris at the end of its tail.",
    "Cassiopeia": "A W of stars opposite the Big Dipper across the pole.",
    "Cygnus": "The swan, flying down the Milky Way.",
    "Lyra": "A small lyre with Vega and the Ring Nebula.",
    "Aquila": "The eagle, holding Altair.",
    "Scorpius": "A curling scorpion low in the summer south.",
    "Sagittarius": "The archer; the centre of the galaxy lies behind it.",
    "Taurus": "The bull, with the Pleiades on its shoulder.",
    "Gemini": "The twins, Castor and Pollux.",
    "Leo": "The lion, a backwards question mark for a head.",
    "Andromeda": "The chained princess, home of the nearest big galaxy.",
    "Perseus": "The hero, with Algol in his hand.",
    "Canis Major": "The great dog, led by Sirius.",
    "Boötes": "The herdsman, a kite shape with Arcturus at its tail.",
    "Crux": "The Southern Cross, the smallest constellation.",
    "Centaurus": "The centaur, wrapped around the Southern Cross.",
}

DEEP_SKY = {
    "Orion Nebula": ("Orion", "A star nursery visible to the naked eye."),
    "Andromeda Galaxy": ("Andromeda", "The farthest thing most people can see without a telescope."),
    "Pleiades": ("Taurus", "The Seven Sisters, a young open cluster."),
    "Ring Nebula": ("Lyra", "A smoke ring left by a dying star."),
    "Lagoon Nebula": ("Sagittarius", "A glowing cloud in the summer Milky Way."),
    "Double Cluster": ("Perseus", "Two open clusters side by side."),
}

ASTERISMS = {
    "Summer Triangle": (["Vega", "Deneb", "Altair"], "Three bright stars from three constellations."),
    "Big Dipper": (["Dubhe", "Merak"], "Part of Ursa Major, and the way to Polaris."),
    "Winter Hexagon": (["Sirius", "Rigel", "Aldebaran", "Pollux", "Castor"], "A ring of bright winter stars."),
}

PLANETS = {
    "Mercury": "Never far from the Sun; look low after sunset.",
    "Venus": "The brightest planet, the evening or morning star.",
    "Mars": "Red, and best when it is opposite the Sun.",
    "Jupiter": "Four moons show in binoculars.",
    "Saturn": "The rings show in any small telescope.",
    "Uranus": "Just visible to the naked eye under a dark sky.",
    "Neptune": "Needs binoculars and a good chart.",
    "Moon": "Best along the line between light and dark, not when full.",
}

GEAR = {
    "Binoculars": "10x50s: the best first instrument.",
    "Telescope": "A small reflector on a simple mount.",
    "Star chart": "A planisphere for the month, and an app as a backup.",
    "Red flashlight": "Keeps the eyes dark-adapted.",
    "Dark site": "Somewhere an hour from the city lights.",
}

SEASONS = {
    "Winter sky": ["Orion", "Taurus", "Gemini", "Canis Major", "Perseus", "Winter Hexagon"],
    "Spring sky": ["Leo", "Boötes", "Ursa Major", "Big Dipper"],
    "Summer sky": ["Lyra", "Cygnus", "Aquila", "Scorpius", "Sagittarius", "Summer Triangle"],
    "Autumn sky": ["Andromeda", "Cassiopeia", "Perseus", "Andromeda Galaxy"],
}


def link(name: str) -> str:
    return f"[[{name}]]"


def main() -> None:
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).with_name("demo-vault.zip")
    rnd = random.Random(7)
    notes: dict[str, str] = {}

    def note(path: str, tags: list[str], body: str) -> None:
        front = "---\ntags: [" + ", ".join(tags) + "]\n---\n" if tags else ""
        notes[path] = front + body.strip() + "\n"

    note("Start here.md", ["hub"], f"""
# Start here

A small stargazing notebook, made to show what Constellate does with a notes folder.
Every note is a dot on the wallpaper and every link a line.

- {link("Sky atlas")}: constellations, stars and deep-sky objects
- {link("Solar System")}: the planets and the Moon
- {link("Observing log")}: one note per night out
- {link("Gear")}: what to take
""")
    note("Sky atlas.md", ["hub"], "# Sky atlas\n\n" + "\n".join(
        f"- {link(c)}" for c in CONSTELLATIONS) + "\n\nBy season: " + ", ".join(link(s) for s in SEASONS)
        + "\n\nShapes that aren't constellations: " + ", ".join(link(a) for a in ASTERISMS))
    note("Solar System.md", ["hub"], "# Solar System\n\n" + "\n".join(f"- {link(p)}" for p in PLANETS))
    note("Gear.md", ["hub"], "# Gear\n\n" + "\n".join(f"- {link(g)}: {d}" for g, d in GEAR.items()))

    for c, line in CONSTELLATIONS.items():
        stars = [s for s, (k, _) in STARS.items() if k == c]
        dso = [d for d, (k, _) in DEEP_SKY.items() if k == c]
        seasons = [s for s, members in SEASONS.items() if c in members]
        body = f"# {c}\n\n{line}\n"
        if stars:
            body += "\nBright stars: " + ", ".join(link(s) for s in stars) + "\n"
        if dso:
            body += "\nWorth finding: " + ", ".join(link(d) for d in dso) + "\n"
        if seasons:
            body += "\nBest in the " + ", ".join(link(s) for s in seasons) + ".\n"
        body += f"\nBack to the {link('Sky atlas')}.\n"
        note(f"Constellations/{c}.md", ["constellation"], body)

    for s, (c, line) in STARS.items():
        note(f"Stars/{s}.md", ["star"], f"# {s}\n\n{line} In {link(c)}.\n")
    for d, (c, line) in DEEP_SKY.items():
        note(f"Deep sky/{d}.md", ["deep-sky"], f"# {d}\n\n{line} In {link(c)}. Try it with {link('Binoculars')} first.\n")
    for a, (members, line) in ASTERISMS.items():
        note(f"Asterisms/{a}.md", ["asterism"], f"# {a}\n\n{line}\n\n" + ", ".join(link(m) for m in members) + "\n")
    for s, members in SEASONS.items():
        note(f"Seasons/{s}.md", ["season"], f"# {s}\n\n" + "\n".join(f"- {link(m)}" for m in members) + "\n")
    for p, line in PLANETS.items():
        note(f"Solar System/{p}.md", ["moon" if p == "Moon" else "planet"], f"# {p}\n\n{line} Part of the {link('Solar System')}.\n")
    for g, line in GEAR.items():
        note(f"Gear/{g}.md", ["gear"], f"# {g}\n\n{line}\n")

    # The observing log: dated nights, each seeing a few objects with some gear.
    targets = list(STARS) + list(DEEP_SKY) + list(PLANETS) + list(CONSTELLATIONS)
    skies = ["clear", "hazy", "clear and cold", "thin cloud later", "windy but clear", "moonlit"]
    days = sorted(rnd.sample(range(5, 300), 22))
    logs = []
    for day in days:
        month, dom = divmod(day, 28)
        date = f"2026-{month + 1:02d}-{dom + 1:02d}"
        seen = rnd.sample(targets, rnd.randint(2, 4))
        gear = rnd.sample(list(GEAR), rnd.randint(1, 2))
        name = f"{date} Observing"
        logs.append(name)
        note(f"Observing log/{name}.md", ["log"], f"""
# {date}

Sky: {rnd.choice(skies)}. With {", ".join(link(g) for g in gear)}.

Seen: {", ".join(link(t) for t in seen)}.
""")
    note("Observing log.md", ["hub"], "# Observing log\n\n" + "\n".join(f"- {link(n)}" for n in logs))

    # An Obsidian vault marker, so the app reads it as one.
    files = {f"{ROOT}/{p}": t for p, t in sorted(notes.items())}
    files[f"{ROOT}/.obsidian/app.json"] = "{}\n"

    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        for path, text in files.items():
            info = zipfile.ZipInfo(path, EPOCH)
            info.external_attr = 0o644 << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            z.writestr(info, text)
    print(f"wrote {out}: {len(notes)} notes")


if __name__ == "__main__":
    main()
