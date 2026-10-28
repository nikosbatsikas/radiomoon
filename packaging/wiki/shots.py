#!/usr/bin/env python3
"""Helpers of the wiki-screenshots workflow.
  shots.py seed <base> <password> <demo-folder>   upload the demo songs, build a playlist, start the first song
  shots.py web  <base> <password> <out-folder>    screenshots of the web studio (Playwright)
"""
import base64, json, os, sys, time, urllib.request, urllib.parse


def call(base, pw, path, body=None, data=None, user="admin"):
    h = {"Authorization": "Basic " + base64.b64encode(f"{user}:{pw}".encode()).decode()}
    if body is not None:
        data = json.dumps(body).encode()
        h["Content-Type"] = "application/json"
    req = urllib.request.Request(base + path, data=data, headers=h, method="POST" if data is not None else "GET")
    with urllib.request.urlopen(req, timeout=120) as r:
        t = r.read()
    try:
        return json.loads(t)
    except Exception:
        return t


def seed(base, pw, demo):
    kinds = {"songs": "music", "jingles": "jingle", "spots": "spot", "sweepers": "sweeper"}
    for folder, kind in kinds.items():
        d = os.path.join(demo, folder)
        if not os.path.isdir(d):
            continue
        for f in sorted(os.listdir(d)):
            q = urllib.parse.urlencode({"name": f, "kind": kind, "read_tags": "true", "parse_filename": "true", "gain": "0", "bookmark": "false", "autocue": "true", "autocue_db": "-30"})
            try:
                call(base, pw, "/api/upload?" + q, data=open(os.path.join(d, f), "rb").read())
            except Exception as e:
                print("upload failed", f, e)
    time.sleep(2)
    call(base, pw, "/api/action", {"do": "reload"})
    songs = call(base, pw, "/api/library?kind=music&limit=50")["items"]
    print("songs:", len(songs))
    for t in songs[:12]:
        call(base, pw, "/api/action", {"do": "add", "id": t["id"], "where": "end"})
    call(base, pw, "/api/action", {"do": "mode", "mode": "manual"})
    call(base, pw, "/api/action", {"do": "play", "index": 0})
    time.sleep(3)
    print(json.dumps(call(base, pw, "/api/state?lite").get("now")))


def web(base, pw, out):
    from playwright.sync_api import sync_playwright
    os.makedirs(out, exist_ok=True)
    with sync_playwright() as p:
        b = p.chromium.launch()
        pg = b.new_page(viewport={"width": 1400, "height": 900})
        pg.goto(base + "/")
        pg.wait_for_selector("#lu")
        pg.wait_for_timeout(1500)
        pg.screenshot(path=f"{out}/web-login.png")
        pg.fill("#lu", "admin")
        pg.fill("#lp", pw)
        pg.click("#lform .go")
        pg.wait_for_selector("#b-out", state="visible")
        pg.wait_for_timeout(4000)
        pg.screenshot(path=f"{out}/web-studio.png")
        pg.click("#b-dl")
        pg.wait_for_timeout(1500)
        pg.screenshot(path=f"{out}/web-download.png")
        pg.reload()
        pg.wait_for_selector("#b-out", state="visible")
        pg.wait_for_timeout(2000)
        pg.click("#b-upd")
        pg.wait_for_timeout(6000)
        pg.screenshot(path=f"{out}/web-update.png")
        b.close()


if __name__ == "__main__":
    if sys.argv[1] == "seed":
        seed(sys.argv[2], sys.argv[3], sys.argv[4])
    else:
        web(sys.argv[2], sys.argv[3], sys.argv[4])
