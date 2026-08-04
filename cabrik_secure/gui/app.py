#!/usr/bin/env python3
import tkinter as tk
from tkinter import ttk, filedialog, messagebox, scrolledtext, simpledialog
import json, os, tempfile, zipfile, shutil, sys

from cabrik_secure.crypto_core import (
    load_identity_keyfile, encrypt_text, encrypt_file, decrypt_text, decrypt_file,
    inspect_metadata, strip_metadata, secure_delete,
    generate_identity, save_identity_keyfile
)

# Build/Version
APP_VERSION = "1.0.0"
BUILD_CHANNEL = "dev"           # z.B. "dev", "beta", "stable"
BUILD_NUMBER = "2025.08.08.1"   # Datum.Zähler o. Git-Commit
BUILD_COPYRIGHT = "© 2025 Cabrik"

# Aus crypto_core Infos anzeigen (Algorithmen/Version/Branding)
from cabrik_secure.crypto_core import ALG as CORE_ALG, VERSION as CORE_VERSION, BRANDING as CORE_BRANDING

APP_TITLE = "Cabrik Secure"
STATE = {
    "keyfile": None,
    "recipient_pub": "",
    "anonymous": True,
    "require_signature": True,
    "attachments": [],
    "last_decrypted_text": None,
}

# ------------------ UI THEME ------------------
def set_dark_theme(root):
    style = ttk.Style(root)
    style.theme_use("clam")

    bg = "#5182AA"
    fg = "#BBF3F0"
    accent = "#1c4d6d"

    root.configure(bg=bg)
    style.configure(".", background=bg, foreground=fg, fieldbackground=bg)
    style.configure("TButton", background=accent, foreground="black", padding=6)
    style.map("TButton", background=[("active", "#275674")])
    style.configure("TNotebook", background=bg)
    style.configure("TNotebook.Tab", background="#2d2d2d", foreground=fg, padding=6)
    style.map("TNotebook.Tab", background=[("selected", accent)])
    style.configure("TEntry", fieldbackground="#7aa39a", foreground=fg)
    style.configure("TLabel", background=bg, foreground=fg)
    style.configure("TFrame", background=bg)
    style.configure("TCheckbutton", background=bg, foreground=fg)

# ---------------- Helpers ----------------
def ask_password(prompt="Passwort für Keyfile:"):
    return simpledialog.askstring("Passwort", prompt, show="*")

def toggle_anonymous():
    STATE["anonymous"] = anon_var.get()
    anon_label_var.set("Anonym senden: EIN" if anon_var.get() else "Anonym senden: AUS")
    CFG["anonymous"] = bool(anon_var.get())
    save_config(CFG)

def on_toggle_sig():
    STATE["require_signature"] = require_sig_var.get()
    CFG["require_signature"] = bool(require_sig_var.get())
    save_config(CFG)

def show_about():
    info = {
        "App": f"{APP_TITLE} v{APP_VERSION} ({BUILD_CHANNEL})",
        "Build": BUILD_NUMBER,
        "Core Version": f"v{CORE_VERSION}",
        "Branding": CORE_BRANDING,
        "Algorithms": f"Asym: {CORE_ALG['asym']} | AEAD: {CORE_ALG['aead']} | KDF: {CORE_ALG['kdf']} | Sig: {CORE_ALG['sig']}",
        "Copyright": BUILD_COPYRIGHT,
    }
    message = "\n".join(f"{k}: {v}" for k, v in info.items())
    messagebox.showinfo(f"Über {APP_TITLE}", message)

def ensure_keyfile_loaded() -> bool:
    """Sorgt dafür, dass STATE['keyfile'] gesetzt ist; fragt ansonsten den User.
       Gibt True zurück, wenn ein Keyfile bereit ist, sonst False."""
    if STATE.get("keyfile") and STATE["keyfile"][2]:
        return True

    # Nutzer fragen: Laden oder Erstellen?
    if messagebox.askyesno("Keyfile benötigt", "Kein Keyfile geladen.\nMöchten Sie jetzt ein vorhandenes Keyfile laden?"):
        choose_keyfile()
    else:
        if messagebox.askyesno("Keyfile erstellen", "Möchten Sie stattdessen ein neues Keyfile erstellen?"):
            create_keyfile()

    return bool(STATE.get("keyfile") and STATE["keyfile"][2])

def decrypt_envelope_string(env_b64: str):
    if not ensure_keyfile_loaded():
        messagebox.showwarning("Abbruch", "Ohne Keyfile kann nicht entschlüsselt werden.")
        return

    ident = STATE["keyfile"][2]
    try:
        text, info = decrypt_text(ident, env_b64.strip(), require_signature=STATE["require_signature"])
        STATE["last_decrypted_text"] = text
        dec_out.delete("1.0", "end")
        dec_out.insert("1.0", text + "\n")
        report = {
            "version": info["header"].get("version"),
            "purpose": info["header"].get("purpose"),
            "ts": info["header"].get("ts"),
            "signature_valid": info.get("signature_valid"),
            "sender_sig_fingerprint": info.get("sender_sig_fingerprint"),
            "meta": info["header"].get("meta"),
        }
        dec_out.insert("end", "\n[Verifikation]\n" + json.dumps(report, indent=2, ensure_ascii=False))
    except Exception:
        # Fallback: Datei-Entschlüsselung
        out_path = filedialog.asksaveasfilename(
            title="Entschlüsselte Datei speichern",
            defaultextension="",
            filetypes=[("Alle Dateien", "*.*")]
        )
        if not out_path:
            return
        info = decrypt_file(ident, env_b64.strip(), out_path, require_signature=STATE["require_signature"])
        STATE["last_decrypted_text"] = None
        dec_out.delete("1.0", "end")
        dec_out.insert("1.0", f"Entschlüsselt → {out_path}\n")
        report = {
            "version": info["header"].get("version"),
            "purpose": info["header"].get("purpose"),
            "ts": info["header"].get("ts"),
            "signature_valid": info.get("signature_valid"),
            "sender_sig_fingerprint": info.get("sender_sig_fingerprint"),
            "meta": info["header"].get("meta"),
        }
        dec_out.insert("end", "\n[Verifikation]\n" + json.dumps(report, indent=2, ensure_ascii=False))

def decrypt_envelope_path(path: str):
    try:
        with open(path, "r", encoding="utf-8") as f:
            env = f.read()
    except Exception as e:
        messagebox.showerror("Fehler", f"Envelope konnte nicht geladen werden:\n{e}")
        return
    decrypt_envelope_string(env)

def _config_dir() -> str:
    if sys.platform.startswith("win"):
        base = os.environ.get("APPDATA") or os.path.expanduser("~\\AppData\\Roaming")
        return os.path.join(base, "CabrikSecure")
    elif sys.platform == "darwin":
        return os.path.expanduser("~/Library/Application Support/CabrikSecure")
    else:
        return os.path.expanduser("~/.config/CabrikSecure")

def _config_path() -> str:
    os.makedirs(_config_dir(), exist_ok=True)
    return os.path.join(_config_dir(), "config.json")

def load_config() -> dict:
    try:
        with open(_config_path(), "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return {}

def save_config(cfg: dict) -> None:
    try:
        with open(_config_path(), "w", encoding="utf-8") as f:
            json.dump(cfg, f, indent=2, ensure_ascii=False)
    except Exception:
        pass

# ---------------- Keyfile Handling ----------------
def choose_keyfile():
    path = filedialog.askopenfilename(
        title="Keyfile wählen",
        filetypes=[("JSON", "*.json"), ("Alle Dateien", "*.*")]
    )
    if not path:
        return
    pwd = ask_password()
    if not pwd:
        messagebox.showwarning("Abbruch", "Kein Passwort eingegeben.")
        return
    try:
        ident = load_identity_keyfile(pwd, path)
        STATE["keyfile"] = (path, pwd, ident)
        key_label_var.set(f"Keyfile: {os.path.basename(path)}")
        messagebox.showinfo("OK", "Keyfile geladen.")
        # Nur hier speichern, wenn es wirklich geklappt hat:
        CFG["last_keyfile"] = path
        save_config(CFG)
    except Exception as e:
        messagebox.showerror("Fehler", f"Keyfile konnte nicht geladen werden:\n{e}")

def create_keyfile():
    use_signing = messagebox.askyesno(
        "Keyfile erstellen",
        "Persistente Signatur erzeugen?\n(Nein = maximale Anonymität)"
    )
    pwd = ask_password("Passwort für neues Keyfile:")
    if not pwd:
        messagebox.showwarning("Abbruch", "Kein Passwort eingegeben.")
        return
    ident = generate_identity(anonymity=not use_signing)
    path = filedialog.asksaveasfilename(
        title="Keyfile speichern",
        defaultextension=".json",
        filetypes=[("JSON", "*.json"), ("Alle Dateien", "*.*")]
    )
    if not path:
        return
    try:
        save_identity_keyfile(ident, pwd, path)
        STATE["keyfile"] = (path, pwd, ident)
        key_label_var.set(f"Keyfile: {os.path.basename(path)}")
        messagebox.showinfo("OK", f"Keyfile gespeichert:\n{path}")
        CFG["last_keyfile"] = path
        save_config(CFG)
    except Exception as e:
        messagebox.showerror("Fehler", f"Speichern fehlgeschlagen:\n{e}")

# ---------------- Senden: Text ----------------
def do_encrypt_text():
    pub = recipient_pub_var.get().strip()
    txt = send_text.get("1.0", "end-1c")
    if not pub:
        messagebox.showwarning("Fehler", "Empfänger Public Key (base64) fehlt.")
        return
    if not txt:
        messagebox.showwarning("Fehler", "Kein Text eingegeben.")
        return

    sender_ident = None
    if not STATE["anonymous"]:
        if not STATE.get("keyfile"):
            messagebox.showwarning("Fehler", "Für persistente Signatur bitte Keyfile laden oder Anonym senden aktivieren.")
            return
        sender_ident = STATE["keyfile"][2]

    try:
        env_b64 = encrypt_text(pub, txt, sender_ident, STATE["anonymous"])
        path = filedialog.asksaveasfilename(
            title="Envelope speichern",
            defaultextension=".enc",
            filetypes=[("Cabrik Envelope", "*.enc"), ("Text", "*.txt"), ("Alle Dateien", "*.*")]
        )
        if not path:
            return
        with open(path, "w", encoding="utf-8") as f:
            f.write(env_b64)
        messagebox.showinfo("Erfolg", f"Verschlüsselt → {path}")
    except Exception as e:
        messagebox.showerror("Fehler", f"Verschlüsselung fehlgeschlagen:\n{e}")

# ---------------- Senden: Anhänge ----------------
def add_attachments(listbox):
    files = filedialog.askopenfilenames(title="Dateien hinzufügen")
    if not files:
        return
    for p in files:
        if p not in STATE["attachments"]:
            STATE["attachments"].append(p)
            listbox.insert("end", p)

def remove_selected(listbox):
    sel = list(listbox.curselection())
    if not sel:
        return
    sel_paths = [listbox.get(i) for i in sel]
    for p in sel_paths:
        if p in STATE["attachments"]:
            STATE["attachments"].remove(p)
    listbox.delete(0, "end")
    for p in STATE["attachments"]:
        listbox.insert("end", p)

def inspect_selected(listbox, show_all: bool):
    sel = list(listbox.curselection())
    if not sel:
        messagebox.showinfo("Info", "Bitte mindestens eine Datei markieren.")
        return
    meta_out.delete("1.0", "end")
    for i in sel:
        path = listbox.get(i)
        try:
            info = inspect_metadata(path)
            if show_all:
                meta_out.insert("end", f"{path}\n{json.dumps(info, indent=2, ensure_ascii=False)}\n\n")
            else:
                compact = {
                    "file": info.get("file"),
                    "type": info.get("type"),
                    "warnings": info.get("warnings"),
                    "keys": list(info.get("metadata", {}).keys())
                }
                meta_out.insert("end", f"{path}\n{json.dumps(compact, indent=2, ensure_ascii=False)}\n\n")
        except Exception as e:
            meta_out.insert("end", f"{path}\n[Fehler beim Inspektieren] {e}\n\n")

def strip_selected(listbox):
    sel = list(listbox.curselection())
    if not sel:
        messagebox.showinfo("Info", "Bitte mindestens eine Datei markieren.")
        return
    changed = []
    for i in sel:
        path = listbox.get(i)
        try:
            clean = strip_metadata(path)
            idx_in_state = STATE["attachments"].index(path)
            STATE["attachments"][idx_in_state] = clean
            listbox.delete(i)
            listbox.insert(i, clean)
            changed.append((path, clean))
        except Exception as e:
            messagebox.showwarning("Fehler", f"{path} konnte nicht bereinigt werden:\n{e}")
    if changed:
        meta_out.insert("end", f"Bereinigt:\n" + "\n".join([f"{a} -> {b}" for a,b in changed]) + "\n")

def encrypt_attachments(listbox):
    pub = recipient_pub_var.get().strip()
    if not pub:
        messagebox.showwarning("Fehler", "Empfänger Public Key (base64) fehlt.")
        return
    if not STATE["attachments"]:
        messagebox.showwarning("Fehler", "Keine Anhänge in der Liste.")
        return

    sender_ident = None
    if not STATE["anonymous"]:
        if not STATE.get("keyfile"):
            messagebox.showwarning("Fehler", "Für persistente Signatur bitte erst Keyfile laden oder Anonym senden aktivieren.")
            return
        sender_ident = STATE["keyfile"][2]

    if len(STATE["attachments"]) == 1:
        path = STATE["attachments"][0]
        use_path = path
        if meta_strip_var.get():
            try:
                use_path = strip_metadata(path)
            except Exception as e:
                messagebox.showerror("Fehler", f"Metadaten-Entfernung fehlgeschlagen:\n{e}")
                return
        try:
            env_b64 = encrypt_file(pub, use_path, sender_ident, STATE["anonymous"])
            path_out = filedialog.asksaveasfilename(
                title="Envelope speichern",
                defaultextension=".enc",
                filetypes=[("Cabrik Envelope", "*.enc"), ("Alle Dateien", "*.*")]
            )
            if not path_out:
                return
            with open(path_out, "w", encoding="utf-8") as f:
                f.write(env_b64)
            messagebox.showinfo("Erfolg", f"Verschlüsselt → {path_out}")
        except Exception as e:
            messagebox.showerror("Fehler", f"Verschlüsselung fehlgeschlagen:\n{e}")
        return

    # Mehrere Anhänge → ZIP bauen
    temp_dir = None
    try:
        temp_dir = tempfile.mkdtemp(prefix="cabrik_")
        zip_path = os.path.join(temp_dir, "attachments.zip")
        with zipfile.ZipFile(zip_path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
            for p in STATE["attachments"]:
                to_add = p
                if meta_strip_var.get():
                    try:
                        to_add = strip_metadata(p)
                    except Exception as e:
                        messagebox.showwarning("Warnung", f"Strip fehlgeschlagen bei {p}:\n{e}\n→ Original wird gepackt.")
                        to_add = p
                zf.write(to_add, arcname=os.path.basename(to_add))

        env_b64 = encrypt_file(pub, zip_path, sender_ident, STATE["anonymous"])
        path_out = filedialog.asksaveasfilename(
            title="Envelope speichern",
            defaultextension=".enc",
            filetypes=[("Cabrik Envelope", "*.enc"), ("Alle Dateien", "*.*")]
        )
        if not path_out:
            return
        with open(path_out, "w", encoding="utf-8") as f:
            f.write(env_b64)
        messagebox.showinfo("Erfolg", f"Verschlüsselt → {path_out}")
    except Exception as e:
        messagebox.showerror("Fehler", f"ZIP/Verschlüsselung fehlgeschlagen:\n{e}")
    finally:
        try:
            if temp_dir:
                shutil.rmtree(temp_dir, ignore_errors=True)
        except Exception:
            pass

# ---------------- Empfangen ----------------
def do_decrypt():
    if not STATE.get("keyfile"):
        messagebox.showwarning("Fehler", "Bitte zuerst Keyfile laden.")
        return

    path = filedialog.askopenfilename(
        title="Envelope wählen",
        filetypes=[("Cabrik Envelope", "*.enc"), ("Alle Dateien", "*.*")]
    )
    if not path:
        return
    try:
        with open(path, "r", encoding="utf-8") as f:
            env = f.read().strip()
    except Exception as e:
        messagebox.showerror("Fehler", f"Datei konnte nicht gelesen werden:\n{e}")
        return

    ident = STATE["keyfile"][2]

    try:
        # Versuchen, Text zu entschlüsseln
        text, info = decrypt_text(ident, env, require_signature=STATE["require_signature"])
        STATE["last_decrypted_text"] = text
        dec_out.delete("1.0", "end")
        dec_out.insert("1.0", text + "\n")
    except Exception:
        # Fallback: Datei-Entschlüsselung
        out_path = filedialog.asksaveasfilename(
            title="Entschlüsselte Datei speichern",
            defaultextension="",
            filetypes=[("Alle Dateien", "*.*")]
        )
        if not out_path:
            return
        info = decrypt_file(ident, env, out_path, require_signature=STATE["require_signature"])
        STATE["last_decrypted_text"] = None
        dec_out.delete("1.0", "end")
        dec_out.insert("1.0", f"Entschlüsselt → {out_path}\n")

    # Verifikationsreport
    report = {
        "version": info["header"].get("version"),
        "purpose": info["header"].get("purpose"),
        "ts": info["header"].get("ts"),
        "signature_valid": info.get("signature_valid"),
        "sender_sig_fingerprint": info.get("sender_sig_fingerprint"),
        "meta": info["header"].get("meta"),
    }
    dec_out.insert("end", "\n[Verifikation]\n" + json.dumps(report, indent=2, ensure_ascii=False))

def save_decrypted_text():
    txt = STATE.get("last_decrypted_text")
    if not txt:
        messagebox.showinfo("Info", "Kein entschlüsselter Text vorhanden.")
        return
    path = filedialog.asksaveasfilename(
        title="Text speichern",
        defaultextension=".txt",
        filetypes=[("Text", "*.txt"), ("Alle Dateien", "*.*")]
    )
    if not path:
        return
    try:
        with open(path, "w", encoding="utf-8") as f:
            f.write(txt)
        messagebox.showinfo("OK", f"Gespeichert: {path}")
    except Exception as e:
        messagebox.showerror("Fehler", f"Speichern fehlgeschlagen:\n{e}")

def do_decrypt_clipboard():
    try:
        encrypted_b64 = root.clipboard_get()
    except tk.TclError:
        messagebox.showerror("Fehler", "Keine Daten in der Zwischenablage gefunden.")
        return

    if not STATE.get("keyfile"):
        messagebox.showwarning("Fehler", "Bitte zuerst Keyfile laden.")
        return

    ident = STATE["keyfile"][2]
    try:
        text, info = decrypt_text(ident, encrypted_b64, require_signature=STATE["require_signature"])
        STATE["last_decrypted_text"] = text
        dec_out.delete("1.0", "end")
        dec_out.insert("1.0", text + "\n")
        report = {
            "version": info["header"].get("version"),
            "purpose": info["header"].get("purpose"),
            "ts": info["header"].get("ts"),
            "signature_valid": info.get("signature_valid"),
            "sender_sig_fingerprint": info.get("sender_sig_fingerprint"),
            "meta": info["header"].get("meta"),
        }
        dec_out.insert("end", "\n[Verifikation]\n" + json.dumps(report, indent=2, ensure_ascii=False))
    except Exception as e:
        messagebox.showerror("Fehler", f"Entschlüsselung fehlgeschlagen:\n{e}")

# ---------------- Werkzeuge ----------------
def do_secure_delete():
    path = filedialog.askopenfilename(title="Datei sicher löschen")
    if not path:
        return
    passes = simpledialog.askinteger("Secure Delete", "Anzahl Überschreib-Pässe:", minvalue=1, initialvalue=3)
    if not passes:
        return
    try:
        secure_delete(path, passes=passes)
        messagebox.showinfo("OK", f"Gelöscht: {path}")
    except Exception as e:
        messagebox.showerror("Fehler", f"Secure Delete fehlgeschlagen:\n{e}")

def do_secure_delete_multi():
    paths = filedialog.askopenfilenames(title="Mehrere Dateien sicher löschen")
    if not paths:
        return
    passes = simpledialog.askinteger("Secure Delete", "Anzahl Überschreib-Pässe:", minvalue=1, initialvalue=3)
    if not passes:
        return

    if not messagebox.askyesno(
        "Bestätigen",
        f"{len(paths)} Datei(en) werden unwiederbringlich überschrieben und gelöscht.\nFortfahren?"
    ):
        return

    ok, fail = [], []
    for p in paths:
        try:
            secure_delete(p, passes=passes)
            if not os.path.exists(p):
                ok.append(p)
            else:
                fail.append((p, "Datei existiert nach dem Löschen weiterhin"))
        except Exception as e:
            fail.append((p, str(e)))

    if ok:
        msg_ok = "\n".join(ok[:10]) + ("\n..." if len(ok) > 10 else "")
    else:
        msg_ok = "—"

    if fail:
        msg_fail = "\n".join([f"{p} – {err}" for p, err in fail[:10]]) + ("\n..." if len(fail) > 10 else "")
    else:
        msg_fail = "—"

    messagebox.showinfo(
        "Secure Delete – Ergebnis",
        f"Erfolgreich gelöscht: {len(ok)}\n{msg_ok}\n\nFehlgeschlagen: {len(fail)}\n{msg_fail}"
    )

# ---------------- GUI Aufbau ----------------
root = tk.Tk()
set_dark_theme(root)
root.title(f"{APP_TITLE} v{APP_VERSION} ({BUILD_CHANNEL})")
root.geometry("1024x720")

CFG = load_config()

# zuletzt genutztes Keyfile auto-laden
_last_keyfile = CFG.get("last_keyfile")
if _last_keyfile and os.path.isfile(_last_keyfile):
    try:
        # Wenn du das Passwort nicht speichern willst (empfohlen!), frag nach:
        pwd = ask_password(f"Passwort für Keyfile\n{_last_keyfile}:")
        if pwd:
            ident = load_identity_keyfile(pwd, _last_keyfile)
            STATE["keyfile"] = (_last_keyfile, pwd, ident)
            # UI Label aktualisieren, wenn es schon existiert – sonst später
    except Exception:
        pass

# Theme/Flags aus Config
STATE["require_signature"] = bool(CFG.get("require_signature", True))
STATE["anonymous"] = bool(CFG.get("anonymous", True))
STATE["recipient_pub"] = CFG.get("recipient_pub", "")

# Statusbar
status_var = tk.StringVar(value=f"{APP_TITLE} v{APP_VERSION} | Core v{CORE_VERSION} | {BUILD_COPYRIGHT}")
status = ttk.Label(root, textvariable=status_var, anchor="w")
status.pack(side="bottom", fill="x")

nb = ttk.Notebook(root)
tab_send = ttk.Frame(nb)
tab_recv = ttk.Frame(nb)
tab_tools = ttk.Frame(nb)
nb.add(tab_send, text="Senden")
nb.add(tab_recv, text="Empfangen")
nb.add(tab_tools, text="Werkzeuge")
nb.pack(fill="both", expand=True)

# Top-Bar
top = ttk.Frame(root)
top.pack(fill="x", padx=8, pady=6)

key_label_var = tk.StringVar(value="Keyfile: (keins)")
ttk.Button(top, text="Keyfile laden…", command=choose_keyfile).pack(side="left")
ttk.Button(top, text="Keyfile erstellen…", command=create_keyfile).pack(side="left", padx=(6,0))
ttk.Label(top, textvariable=key_label_var).pack(side="left", padx=8)

# Falls beim Start via Autoload bereits gesetzt:
if STATE.get("keyfile"):
    key_label_var.set(f"Keyfile: {os.path.basename(STATE['keyfile'][0])}")

ttk.Label(top, text="Empfänger ENC Public Key (base64):").pack(side="left", padx=(24, 4))
recipient_pub_var = tk.StringVar()
ttk.Entry(top, textvariable=recipient_pub_var, width=56).pack(side="left", padx=4)

anon_var = tk.BooleanVar(value=True)
anon_label_var = tk.StringVar(value="Anonym senden: EIN")
# Anonym-Initialzustand aus STATE/CFG
anon_var.set(STATE["anonymous"])
anon_label_var.set("Anonym senden: EIN" if STATE["anonymous"] else "Anonym senden: AUS")
ttk.Checkbutton(top, textvariable=anon_label_var, variable=anon_var, command=toggle_anonymous).pack(side="right")

# Menüleiste
menubar = tk.Menu(root)
root.config(menu=menubar)

m_datei = tk.Menu(menubar, tearoff=0)
m_hilfe = tk.Menu(menubar, tearoff=0)

# kleine Shortcuts
m_datei.add_command(label="Beenden", command=root.quit)
m_hilfe.add_command(label=f"Über {APP_TITLE}", command=show_about)

menubar.add_cascade(label="Datei", menu=m_datei)
menubar.add_cascade(label="Hilfe", menu=m_hilfe)

# Tab Senden
left = ttk.Frame(tab_send)
right = ttk.Frame(tab_send)
left.pack(side="left", fill="both", expand=True, padx=8, pady=8)
right.pack(side="left", fill="both", expand=True, padx=8, pady=8)

# Text senden
ttk.Label(left, text="Textnachricht:").pack(anchor="w")
send_text = scrolledtext.ScrolledText(left, wrap="word", height=16)
send_text.pack(fill="both", expand=True)
ttk.Button(left, text="Text verschlüsseln…", command=do_encrypt_text).pack(pady=6)

# Anhänge
att_frame = ttk.LabelFrame(right, text="Anhänge")
att_frame.pack(fill="both", expand=True)
att_list = tk.Listbox(att_frame, selectmode="extended", height=10)
att_list.pack(fill="both", expand=True, padx=6, pady=(6,0))
btns1 = ttk.Frame(att_frame); btns1.pack(fill="x", padx=6, pady=6)
ttk.Button(btns1, text="Hinzufügen…", command=lambda: add_attachments(att_list)).pack(side="left", padx=4)
ttk.Button(btns1, text="Entfernen", command=lambda: remove_selected(att_list)).pack(side="left", padx=4)

meta_strip_var = tk.BooleanVar(value=False)
ttk.Checkbutton(att_frame, text="Beim Senden Metadaten strippen", variable=meta_strip_var).pack(padx=6, pady=(0,6))
meta_out = scrolledtext.ScrolledText(att_frame, wrap="word", height=8)
meta_out.pack(fill="both", expand=True, padx=6, pady=(0,6))
btns2 = ttk.Frame(att_frame); btns2.pack(fill="x", padx=6, pady=(0,6))
ttk.Button(btns2, text="Metadaten ansehen", command=lambda: inspect_selected(att_list, False)).pack(side="left", padx=4)
ttk.Button(btns2, text="Alle Metadaten", command=lambda: inspect_selected(att_list, True)).pack(side="left",  padx=4)
ttk.Button(btns2, text="Metadaten strippen", command=lambda: strip_selected(att_list)).pack(side="left", padx=4)

ttk.Button(att_frame, text="Anhänge verschlüsseln…", command=lambda: encrypt_attachments(att_list)).pack(pady=6)

# Tab Empfangen
recv_top = ttk.Frame(tab_recv); recv_top.pack(fill="x", padx=8, pady=(8,0))
ttk.Label(recv_top, text="Entschlüsselte Ausgabe / Verifikation:").pack(side="left")
ttk.Button(recv_top, text="Envelope öffnen & entschlüsseln…", command=do_decrypt).pack(side="right", padx=(6,0))
ttk.Button(recv_top, text="Aus Zwischenablage entschlüsseln", command=do_decrypt_clipboard).pack(side="right", padx=(6,0))
ttk.Button(recv_top, text="Text speichern…", command=save_decrypted_text).pack(side="right", padx=(6,0))
require_sig_var = tk.BooleanVar(value=STATE["require_signature"])
ttk.Checkbutton(
    tab_recv,
    text="Signatur erforderlich",
    variable=require_sig_var,
    command=on_toggle_sig
).pack(anchor="w", padx=8, pady=4)

dec_out = scrolledtext.ScrolledText(tab_recv, wrap="word", height=28)
dec_out.pack(fill="both", expand=True, padx=8, pady=8)

def on_recipient_changed(*_):
    CFG["recipient_pub"] = recipient_pub_var.get().strip()
    save_config(CFG)

recipient_pub_var.trace_add("write", on_recipient_changed)
recipient_pub_var.set(STATE["recipient_pub"])  # aus CFG initial anzeigen

# Optional: Envelope per Doppelklick öffnen (Dateizuordnung)
_pending_arg_path = None
if len(sys.argv) > 1:
    p = sys.argv[1]
    if os.path.isfile(p):
        _pending_arg_path = p

def _open_pending_envelope():
    if _pending_arg_path:
        decrypt_envelope_path(_pending_arg_path)

# Nach UI-Init, aber noch vor mainloop ausführen:
root.after(250, _open_pending_envelope)

# Tab Werkzeuge
tools_btns = ttk.Frame(tab_tools)
tools_btns.pack(padx=8, pady=12, fill="x")
ttk.Button(tools_btns, text="Secure Delete (einzeln)…", command=do_secure_delete).pack(side="left", padx=(0,6))
ttk.Button(tools_btns, text="Secure Delete (mehrere)…", command=do_secure_delete_multi).pack(side="left")

root.mainloop()
