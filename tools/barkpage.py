"""Writes _inbox/barkpage.js: everything the AI Studio speech page needs to record battle lines alone.

    python tools/barkpage.py

The script carries every line not yet recorded (docs/prompts/voice/barks.<lang>.txt minus the clips
in assets/voice/<lang>/), the casting, and an in-page loop. Load it into the page (a file input and
the browser tool's upload), then:

    tropaStyle('volkolak', 'ru')            // the style (Scene) text for that speaker in that language
    await tropaRecord('volkolak', 6, 'ru')  // the next six missing lines of that speaker in that language

One language per run: each language needs its own style, or the accents bleed into each other
(modern Russian read the old way, English without its Slavic accent).

Each finished clip is saved straight from the page as `tropa__<lang>__<key>.wav`, so several tabs
can record at once; take them in with `python tools/voice_in.py --named`. Pick the speaker's voice
(barks.voices.txt) and fill the Style box by hand first: the page ignores a style set by script.
"""
import json
import os
import re

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
VOICE = os.path.join(ROOT, "docs", "prompts", "voice")

LOOP = r"""
window.TROPA_LANG={'ru-old':'The line is in Old Russian, read in the solemn old manner: every vowel as written, unstressed O stays a round O, E never read as YO; a capital letter inside a word marks its stressed vowel.','ru':'The line is in plain modern Russian: ordinary present-day pronunciation and natural stress, nothing archaic or churchly. The capital letters are only the start of sentences.','en':'The line is in English, spoken by a Slav: a clear, noticeable Russian accent, rolled R, heavy consonants.'};
window.tropaStyle=(who,lang)=>barkVoice[who].persona+' '+TROPA_LANG[lang]+' Speak only the line, once.';
window.tropaSet=(text)=>{ const t=[...document.querySelectorAll('textarea')].find(x=>(x.getAttribute('aria-label')||'')==='Speech block text'); const set=Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set; set.call(t,text); t.dispatchEvent(new Event('input',{bubbles:true})); t.dispatchEvent(new Event('change',{bubbles:true})); return t; };
window.tropaDone=()=>{ try { return JSON.parse(localStorage.tropaDone||'[]'); } catch(e) { return []; } };
window.tropaTodo=(who,only)=>{ const done=new Set(tropaDone()); const out=[]; for(const lang of (only?[only]:['ru-old','ru','en'])) for(const k of Object.keys(barkQ[lang]||{})) if(k.split('-')[1]===who && !done.has(lang+'/'+k)) out.push([lang,k]); return out; };
window.tropaRecord=async (who,n,only)=>{
  const sleep=ms=>new Promise(r=>setTimeout(r,ms));
  const log=[];
  // never record with the page's default voice: the speaker chosen must be this hero's
  const want=barkVoice[who].voice;
  if(!new RegExp('Speaker 1 - '+want+'\\b').test(document.body.innerText)) return 'WRONG VOICE: pick '+want+' first';
  for(const [lang,key] of tropaTodo(who,only).slice(0,n)){
    if(/Quota limit/.test(document.body.innerText)) { log.push('QUOTA'); break; }
    // a run that hangs shows Stop instead of Run: give it time, then stop it
    const runButton=()=>[...document.querySelectorAll('button')].find(b=>/^Run/.test(b.textContent.trim()));
    for(let i=0;i<60 && !runButton();i++){
      await sleep(1000);
      if(i===40){ const stop=[...document.querySelectorAll('button')].find(b=>/^Stop/.test(b.textContent.trim())); if(stop) stop.click(); }
    }
    if(!runButton()){ log.push('STUCK'); break; }
    const a=document.querySelector('audio'); const before=a?a.src:'';
    tropaSet(barkQ[lang][key]);
    await sleep(300);
    runButton().click();
    // the clip streams in: it is done when it is new, longer than a bare header, the Run button is
    // back, and it has stopped growing for a second and a half
    let ok=false, last=-1, still=0;
    for(let i=0;i<150;i++){
      await sleep(400);
      if(/Quota limit/.test(document.body.innerText)) break;
      const au=document.querySelector('audio');
      const r=[...document.querySelectorAll('button')].find(b=>/^Run/.test(b.textContent.trim()));
      const len=au&&au.src&&au.src!==before?au.src.length:0;
      still=(len>20000 && len===last && r && !r.disabled)?still+1:0;
      last=len;
      if(still>=4){ ok=true; break; }
    }
    if(!ok){ log.push('FAIL '+lang+' '+key); if(/Quota limit/.test(document.body.innerText)){ log.push('QUOTA'); break; } continue; }
    await sleep(500);
    const au=document.querySelector('audio'); au.pause();
    const link=document.createElement('a'); link.href=au.src; link.download='tropa__'+lang+'__'+key+'.wav'; document.body.appendChild(link); link.click(); link.remove();
    const d=tropaDone(); d.push(lang+'/'+key); localStorage.tropaDone=JSON.stringify(d);
    log.push(lang+' '+key);
    await sleep(800);
  }
  return log.join(' | ')+' || left '+tropaTodo(who,only).length;
};
"""


def main():
    lines = {}
    for lang in ("ru-old", "ru", "en"):
        text = open(os.path.join(VOICE, f"barks.{lang}.txt"), encoding="utf-8").read()
        lines[lang] = {k: v for k, v in re.findall(r"^(bark-[a-z0-9-]+) = (.*)$", text, re.M)
                       if not os.path.exists(os.path.join(ROOT, "assets", "voice", lang, f"{k}.wav"))}
    voices = {}
    for line in open(os.path.join(VOICE, "barks.voices.txt"), encoding="utf-8"):
        m = re.match(r"^([a-z_]+) = (\w+) \| (.*)$", line.rstrip("\r\n"))
        if m:
            voices[m.group(1)] = {"voice": m.group(2), "persona": m.group(3)}
    js = f"window.barkQ={json.dumps(lines, ensure_ascii=False)};\nwindow.barkVoice={json.dumps(voices, ensure_ascii=False)};\n{LOOP}"
    os.makedirs(os.path.join(ROOT, "_inbox"), exist_ok=True)
    out = os.path.join(ROOT, "_inbox", "barkpage.js")
    open(out, "w", encoding="utf-8").write(js)
    print(out, len(js), {k: len(v) for k, v in lines.items()})


if __name__ == "__main__":
    main()
