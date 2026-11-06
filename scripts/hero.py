# README hero: the photo-filtered wall behind, the bench mid-compression in front.
# Run after scripts/screenshots.mjs.
import subprocess, os
os.chdir(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "docs"))
def rounded(r):
    return ["-alpha", "set", "(", "+clone", "-alpha", "extract", "-draw", f"fill black polygon 0,0 0,{r} {r},0 fill white circle {r},{r} {r},0",
            "(", "+clone", "-flip", ")", "-compose", "Multiply", "-composite", "(", "+clone", "-flop", ")", "-compose", "Multiply", "-composite", ")",
            "-alpha", "off", "-compose", "CopyOpacity", "-composite", "-compose", "over"]
def card(src, width, out):
    a = ["magick", src, "-resize", f"{width}x"] + rounded(22) + [
        "(", "+clone", "-background", "#2a1e10", "-shadow", "38x30+0+26", ")", "+swap", "-background", "none", "-layers", "merge", "+repage", out]
    subprocess.run(a, check=True)
card("wall.png", 1820, "/tmp/hero-back.png")
card("bench.png", 1340, "/tmp/hero-front.png")
subprocess.run(["magick", "-size", "2700x1640", "gradient:#efe4cd-#d9c5a0",
    "/tmp/hero-back.png", "-geometry", "+110+90", "-composite",
    "/tmp/hero-front.png", "-geometry", "+1240+690", "-composite",
    "-resize", "1800x", "-strip", "-define", "png:compression-level=9", "hero.png"], check=True)
print(subprocess.run(["magick", "identify", "-format", "%wx%h %b", "hero.png"], capture_output=True, text=True).stdout)
