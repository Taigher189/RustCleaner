"""
RustCleaner Visual Asset Generator
Generates high-resolution screenshots, hero banner, and an animated demo GIF
matching the exact Obsidian Dark theme palette of RustCleaner.
"""

import os
from PIL import Image, ImageDraw, ImageFont

ASSETS_DIR = r"C:\Users\128ma\.gemini\antigravity\scratch\rustcleaner\assets"
os.makedirs(ASSETS_DIR, exist_ok=True)

# Colors matching ThemePalette (Obsidian Dark)
BG_CANVAS = (11, 15, 23)        # #0B0F17
BG_SIDEBAR = (15, 21, 32)       # #0F1520
BG_HEADER = (19, 26, 38)        # #131A26
BG_CARD = (22, 30, 44)          # #161E2C
BG_CARD_SUBTLE = (26, 36, 52)   # #1A2434
BORDER_SUBTLE = (42, 54, 76)    # #2A364C
BORDER_STRONG = (70, 88, 120)   # #465878

TEXT_PRIMARY = (248, 250, 252)  # #F8FAFC
TEXT_SECONDARY = (148, 163, 184)# #94A3B8
TEXT_MUTED = (100, 116, 139)    # #64748B

ACCENT_RUST = (234, 88, 12)     # #EA580C
ACCENT_EMERALD = (16, 185, 129) # #10B981
ACCENT_CYAN = (6, 182, 212)     # #06B6D4
ACCENT_AMBER = (245, 158, 11)   # #F59E0B
ACCENT_PURPLE = (168, 85, 247)  # #A855F7

# Fonts
FONT_TITLE = ImageFont.truetype("C:/Windows/Fonts/segoeuib.ttf", 22)
FONT_HEADING = ImageFont.truetype("C:/Windows/Fonts/segoeuib.ttf", 16)
FONT_SUBTITLE = ImageFont.truetype("C:/Windows/Fonts/segoeui.ttf", 13)
FONT_BODY = ImageFont.truetype("C:/Windows/Fonts/segoeui.ttf", 13)
FONT_BODY_BOLD = ImageFont.truetype("C:/Windows/Fonts/segoeuib.ttf", 13)
FONT_SMALL = ImageFont.truetype("C:/Windows/Fonts/segoeui.ttf", 11)
FONT_MONO = ImageFont.truetype("C:/Windows/Fonts/consola.ttf", 12)
FONT_KPI_VAL = ImageFont.truetype("C:/Windows/Fonts/segoeuib.ttf", 24)
FONT_HERO_TITLE = ImageFont.truetype("C:/Windows/Fonts/segoeuib.ttf", 44)
FONT_HERO_SUB = ImageFont.truetype("C:/Windows/Fonts/segoeui.ttf", 20)

W, H = 1200, 740

def draw_pill(draw, x, y, text, fg_color, bg_alpha_color, font=FONT_SMALL):
    bbox = font.getbbox(text)
    tw = bbox[2] - bbox[0]
    th = bbox[3] - bbox[1]
    pad_x, pad_y = 8, 4
    rw = tw + pad_x * 2
    rh = th + pad_y * 2
    draw.rounded_rectangle([x, y, x + rw, y + rh], radius=6, fill=bg_alpha_color, outline=fg_color, width=1)
    draw.text((x + pad_x, y + pad_y - 1), text, fill=fg_color, font=font)
    return rw

def draw_sidebar(draw, active_tab_name):
    # Sidebar Background
    draw.rectangle([0, 0, 240, H], fill=BG_SIDEBAR)
    draw.line([240, 0, 240, H], fill=BORDER_SUBTLE, width=1)

    # Logo & App Title
    draw.rounded_rectangle([20, 20, 52, 52], radius=10, fill=ACCENT_RUST)
    draw.text((28, 22), "RC", fill=(255, 255, 255), font=FONT_TITLE)
    draw.text((64, 20), "RustCleaner", fill=TEXT_PRIMARY, font=FONT_HEADING)
    draw.text((64, 38), "Zero-Telemetry v1.4.0", fill=TEXT_MUTED, font=FONT_SMALL)

    draw.line([20, 68, 220, 68], fill=BORDER_SUBTLE, width=1)

    # Nav Items
    items = [
        ("Pulizia Rapida & Privacy", "Cache, temp e registri", ACCENT_RUST),
        ("Analisi Disco & Duplicati", "SHA-256 e file giganti", ACCENT_CYAN),
        ("Registro di Windows", "Pulizia chiavi orfane", ACCENT_AMBER),
        ("Gestione Avvio", "App e task automatici", ACCENT_CYAN),
        ("Disinstallazione", "Uninstaller & Residuals", ACCENT_PURPLE),
        ("Privacy & Decrapifier", "Telemetria & Win 11 Debloat", ACCENT_EMERALD),
        ("Drive Wiper Sicuro", "Bonifica spazio libero", ACCENT_EMERALD),
        ("Aggiornamento Software", "Winget & driver update", ACCENT_CYAN),
        ("Monitoraggio Zero-RAM", "Pianificazione silenziosa", ACCENT_PURPLE),
    ]

    y = 80
    for name, sub, color in items:
        is_active = (name == active_tab_name)
        if is_active:
            draw.rounded_rectangle([12, y, 228, y + 48], radius=8, fill=BG_CARD_SUBTLE, outline=color, width=1)
        else:
            draw.rounded_rectangle([12, y, 228, y + 48], radius=8, fill=BG_SIDEBAR)

        # Indicator dot
        dot_color = color if is_active else TEXT_MUTED
        draw.ellipse([24, y + 18, 34, y + 28], fill=dot_color)

        draw.text((44, y + 8), name, fill=TEXT_PRIMARY if is_active else TEXT_SECONDARY, font=FONT_BODY_BOLD if is_active else FONT_BODY)
        draw.text((44, y + 26), sub, fill=TEXT_MUTED, font=FONT_SMALL)
        y += 54

    # Sidebar Bottom Badge
    draw.rounded_rectangle([16, H - 60, 224, H - 18], radius=8, fill=BG_CARD, outline=BORDER_SUBTLE, width=1)
    draw.text((28, H - 52), "ZERO TELEMETRIA", fill=ACCENT_EMERALD, font=FONT_BODY_BOLD)
    draw.text((28, H - 34), "0 MB RAM in background", fill=TEXT_MUTED, font=FONT_SMALL)

def draw_header(draw, title, subtitle):
    # Header Background
    draw.rectangle([240, 0, W, 70], fill=BG_HEADER)
    draw.line([240, 70, W, 70], fill=BORDER_SUBTLE, width=1)

    draw.text((260, 14), title, fill=TEXT_PRIMARY, font=FONT_TITLE)
    draw.text((260, 42), subtitle, fill=TEXT_SECONDARY, font=FONT_SUBTITLE)

    # Top right badges
    rx = W - 280
    draw_pill(draw, rx, 20, "ADMIN: ELEVATO", ACCENT_EMERALD, (16, 185, 129, 40))
    draw_pill(draw, rx + 115, 20, "v1.4.0-alpha", ACCENT_RUST, (234, 88, 12, 40))
    draw_pill(draw, rx + 205, 20, "IT - Italiano", ACCENT_CYAN, (6, 182, 212, 40))

def draw_kpi_card(draw, x, y, w, h, label, value, sub, val_color):
    draw.rounded_rectangle([x, y, x + w, y + h], radius=10, fill=BG_CARD, outline=BORDER_SUBTLE, width=1)
    draw.text((x + 16, y + 14), label, fill=TEXT_MUTED, font=FONT_SMALL)
    draw.text((x + 16, y + 34), value, fill=val_color, font=FONT_KPI_VAL)
    draw.text((x + 16, y + 68), sub, fill=TEXT_SECONDARY, font=FONT_SMALL)

# ----------------------------------------------------------------------------
# 1. SCREENSHOT: PULIZIA RAPIDA
# ----------------------------------------------------------------------------
def create_cleaner_screenshot():
    img = Image.new("RGB", (W, H), BG_CANVAS)
    draw = ImageDraw.Draw(img)
    draw_sidebar(draw, "Pulizia Rapida & Privacy")
    draw_header(draw, "Pulizia File System & Privacy", "Analisi ed eliminazione sicura di cache, file temporanei e tracce browser.")

    # Action Buttons
    draw.rounded_rectangle([260, 85, 410, 122], radius=8, fill=ACCENT_RUST)
    draw.text((285, 94), "Avvia Analisi", fill=(255, 255, 255), font=FONT_BODY_BOLD)

    draw.rounded_rectangle([425, 85, 580, 122], radius=8, fill=ACCENT_EMERALD)
    draw.text((445, 94), "Pulisci Sistema", fill=(255, 255, 255), font=FONT_BODY_BOLD)

    # KPIs
    draw_kpi_card(draw, 260, 135, 290, 95, "SPAZIO LIBERABILE", "1.42 GB", "8,412 file temporanei", ACCENT_EMERALD)
    draw_kpi_card(draw, 570, 135, 290, 95, "REGOLE ATTIVE", "16 / 16", "Windows, Browser & App", ACCENT_CYAN)
    draw_kpi_card(draw, 880, 135, 290, 95, "STATO SICUREZZA", "100% SICURO", "File in uso ignorati", ACCENT_PURPLE)

    # Table Card
    draw.rounded_rectangle([260, 245, W - 30, H - 25], radius=10, fill=BG_CARD, outline=BORDER_SUBTLE, width=1)
    draw.text((280, 260), "DETTAGLIO REGOLE DI PULIZIA SCANSIONATE", fill=TEXT_PRIMARY, font=FONT_HEADING)

    rules = [
        ("File Temporanei Utente (%TEMP%)", "412.5 MB", "3,210 file", "Pronto per l'eliminazione", ACCENT_EMERALD),
        ("Cache Google Chrome (Default & GPU)", "385.2 MB", "1,840 file", "Sessione chiusa, sicuro da pulire", ACCENT_EMERALD),
        ("Cache Microsoft Edge & Webview", "240.1 MB", "980 file", "File di navigazione e dati GPU", ACCENT_EMERALD),
        ("File Temporanei di Sistema Windows", "198.4 MB", "715 file", "C:\\Windows\\Temp", ACCENT_EMERALD),
        ("Cestino di Windows ($Recycle.Bin)", "112.0 MB", "48 file", "Tutti i dischi locali", ACCENT_EMERALD),
        ("Dump di Crash e Rapporti di Errore", "68.9 MB", "32 file", "WER e minidump orfani", ACCENT_EMERALD),
        ("Cache Icone e Anteprime Esplora Risorse", "24.5 MB", "12 file", "IconCache.db e thumbcache", ACCENT_EMERALD),
    ]

    ty = 295
    for name, size, files, status, col in rules:
        draw.rounded_rectangle([280, ty, W - 50, ty + 50], radius=6, fill=BG_CARD_SUBTLE, outline=BORDER_SUBTLE, width=1)
        draw.text((300, ty + 15), name, fill=TEXT_PRIMARY, font=FONT_BODY_BOLD)
        draw.text((700, ty + 15), files, fill=TEXT_MUTED, font=FONT_SMALL)
        draw.text((820, ty + 13), size, fill=ACCENT_CYAN, font=FONT_BODY_BOLD)
        draw_pill(draw, 960, ty + 13, status, col, (col[0], col[1], col[2], 30))
        ty += 56

    return img

# ----------------------------------------------------------------------------
# 2. SCREENSHOT: DISK ANALYZER & SHA-256 DUPLICATES
# ----------------------------------------------------------------------------
def create_disk_analyzer_screenshot():
    img = Image.new("RGB", (W, H), BG_CANVAS)
    draw = ImageDraw.Draw(img)
    draw_sidebar(draw, "Analisi Disco & Duplicati")
    draw_header(draw, "Analisi Spazio Disco & Duplicati SHA-256", "Motore multi-stadio a zero sprechi: Byte Size -> Fingerprint 8 KB -> SHA-256 crittografico.")

    # Action Buttons
    draw.rounded_rectangle([260, 85, 410, 122], radius=8, fill=ACCENT_CYAN)
    draw.text((285, 94), "Scansiona Disco", fill=(255, 255, 255), font=FONT_BODY_BOLD)

    draw.rounded_rectangle([425, 85, 680, 122], radius=8, fill=ACCENT_RUST)
    draw.text((440, 94), "Elimina Duplicati Selezionati", fill=(255, 255, 255), font=FONT_BODY_BOLD)

    # KPIs
    draw_kpi_card(draw, 260, 135, 290, 95, "SPAZIO ANALIZZATO", "485.2 GB", "124,510 file scansionati", ACCENT_CYAN)
    draw_kpi_card(draw, 570, 135, 290, 95, "FILE GIGANTI (>50 MB)", "250 file", "Totale 86.4 GB", ACCENT_AMBER)
    draw_kpi_card(draw, 880, 135, 290, 95, "SPAZIO SPRECATO DUPLICATI", "14.8 GB", "38 gruppi identici (SHA-256)", ACCENT_RUST)

    # Duplicates Card Table
    draw.rounded_rectangle([260, 245, W - 30, H - 25], radius=10, fill=BG_CARD, outline=BORDER_SUBTLE, width=1)
    draw.text((280, 260), "GRUPPI DI DUPLICATI SHA-256 RILEVATI (1 ORIGINALE PROTETTO AUTOMATICAMENTE)", fill=TEXT_PRIMARY, font=FONT_HEADING)

    dup_groups = [
        ("Windows11_24H2_English_x64.iso (5.42 GB)", "Hash SHA-256: 7d8f...4e1a", [
            ("C:\\Downloads\\Windows11_24H2_English_x64.iso", "ORIGINALE PROTETTO", ACCENT_EMERALD, False),
            ("D:\\Backup\\ISOs\\Windows11_24H2_English_x64 (1).iso", "COPIA DUPLICATA (5.42 GB sprecati)", ACCENT_RUST, True),
        ]),
        ("dataset_training_weights_v4.tar.gz (3.18 GB)", "Hash SHA-256: 3c91...88b2", [
            ("C:\\Users\\manu\\Projects\\dataset_training_weights_v4.tar.gz", "ORIGINALE PROTETTO", ACCENT_EMERALD, False),
            ("C:\\Downloads\\dataset_training_weights_v4.tar.gz", "COPIA DUPLICATA (3.18 GB sprecati)", ACCENT_RUST, True),
        ]),
    ]

    ty = 295
    for grp_title, grp_hash, files in dup_groups:
        draw.rounded_rectangle([280, ty, W - 50, ty + 120], radius=8, fill=BG_CARD_SUBTLE, outline=BORDER_STRONG, width=1)
        draw.text((300, ty + 10), grp_title, fill=TEXT_PRIMARY, font=FONT_BODY_BOLD)
        draw.text((750, ty + 12), grp_hash, fill=TEXT_MUTED, font=FONT_MONO)

        sub_y = ty + 38
        for path, badge_text, badge_col, checked in files:
            draw.rectangle([300, sub_y + 3, 314, sub_y + 17], outline=TEXT_MUTED, width=1, fill=ACCENT_RUST if checked else BG_CARD)
            draw.text((324, sub_y + 2), path, fill=TEXT_SECONDARY, font=FONT_MONO)
            draw_pill(draw, W - 320, sub_y, badge_text, badge_col, (badge_col[0], badge_col[1], badge_col[2], 30))
            sub_y += 32

        ty += 135

    return img

# ----------------------------------------------------------------------------
# 3. SCREENSHOT: DEEP RESIDUAL HUNTER
# ----------------------------------------------------------------------------
def create_residual_hunter_screenshot():
    img = Image.new("RGB", (W, H), BG_CANVAS)
    draw = ImageDraw.Draw(img)
    draw_sidebar(draw, "Disinstallazione")
    draw_header(draw, "Deep Residual Hunter (Cacciatore Residui Orfani)", "Individua ed elimina file, cartelle e chiavi di registro lasciate indietro dai disinstallatori.")

    # Action Buttons
    draw.rounded_rectangle([260, 85, 430, 122], radius=8, fill=BG_CARD, outline=BORDER_SUBTLE, width=1)
    draw.text((280, 94), "< Torna alla Lista App", fill=TEXT_SECONDARY, font=FONT_BODY_BOLD)

    draw.rounded_rectangle([445, 85, 820, 122], radius=8, fill=ACCENT_RUST)
    draw.text((465, 94), "Elimina Residui & Crea Backup .reg", fill=(255, 255, 255), font=FONT_BODY_BOLD)

    # KPIs
    draw_kpi_card(draw, 260, 135, 290, 95, "ELEMENTI RESIDUI", "28 trovati", "Cartelle, file e chiavi", ACCENT_CYAN)
    draw_kpi_card(draw, 570, 135, 290, 95, "SPAZIO RECUPERABILE", "384.5 MB", "Orfano su disco fisso", ACCENT_EMERALD)
    draw_kpi_card(draw, 880, 135, 290, 95, "CHIAVI DI REGISTRO", "14 chiavi", "Backup .reg automatico", ACCENT_PURPLE)

    # Residuals Table
    draw.rounded_rectangle([260, 245, W - 30, H - 25], radius=10, fill=BG_CARD, outline=BORDER_SUBTLE, width=1)
    draw.text((280, 260), "RESIDUI DI: 'Spotify Music (SpotifyAB)' DOPO LA DISINSTALLAZIONE", fill=TEXT_PRIMARY, font=FONT_HEADING)

    residuals = [
        ("CARTELLA", "C:\\Users\\manu\\AppData\\Local\\Spotify", "342.1 MB", "Alta Sicurezza", "Cartella cache e dati orfana", ACCENT_CYAN),
        ("CARTELLA", "C:\\Users\\manu\\AppData\\Roaming\\Spotify", "42.4 MB", "Alta Sicurezza", "Configurazioni e credenziali locali", ACCENT_CYAN),
        ("REGISTRO", "HKCU\\Software\\Spotify", "Chiave", "Alta Sicurezza", "Albero di configurazione software", ACCENT_PURPLE),
        ("REGISTRO", "HKCU\\Software\\Classes\\spotify", "Protocollo", "Alta Sicurezza", "Gestore URI protocollo orfano", ACCENT_PURPLE),
        ("REGISTRO", "HKLM\\SOFTWARE\\WOW6432Node\\Spotify", "Chiave", "Media Sicurezza", "Sottochiave registro a 32-bit", ACCENT_PURPLE),
        ("FILE", "C:\\Users\\manu\\AppData\\Local\\Temp\\spotify_setup.log", "12 KB", "Alta Sicurezza", "Log temporaneo di installazione", ACCENT_AMBER),
    ]

    ty = 295
    for kind, path, size, conf, reason, col in residuals:
        draw.rounded_rectangle([280, ty, W - 50, ty + 54], radius=6, fill=BG_CARD_SUBTLE, outline=BORDER_SUBTLE, width=1)
        draw.rectangle([295, ty + 18, 309, ty + 32], outline=TEXT_MUTED, fill=ACCENT_EMERALD)

        draw_pill(draw, 325, ty + 14, kind, col, (col[0], col[1], col[2], 30))
        draw_pill(draw, 415, ty + 14, conf, ACCENT_EMERALD, (16, 185, 129, 30))

        draw.text((540, ty + 10), path, fill=TEXT_PRIMARY, font=FONT_MONO)
        draw.text((540, ty + 28), reason, fill=TEXT_MUTED, font=FONT_SMALL)

        draw.text((W - 140, ty + 16), size, fill=ACCENT_EMERALD, font=FONT_BODY_BOLD)
        ty += 60

    return img

# ----------------------------------------------------------------------------
# 4. SCREENSHOT: WINDOWS 11 PRIVACY & DECRAPIFIER
# ----------------------------------------------------------------------------
def create_decrapifier_screenshot():
    img = Image.new("RGB", (W, H), BG_CANVAS)
    draw = ImageDraw.Draw(img)
    draw_sidebar(draw, "Privacy & Decrapifier")
    draw_header(draw, "Windows 11 Privacy Fortress & Decrapifier", "Disattiva telemetria invasiva, ricerca Bing nel menu Start, widget MSN e bloatware.")

    # Action Buttons
    draw.rounded_rectangle([260, 85, 540, 122], radius=8, fill=ACCENT_EMERALD)
    draw.text((280, 94), "Applica Ottimizzazione Consigliata", fill=(255, 255, 255), font=FONT_BODY_BOLD)

    draw.rounded_rectangle([555, 85, 840, 122], radius=8, fill=BG_CARD, outline=BORDER_SUBTLE, width=1)
    draw.text((575, 94), "Ripristina Default Windows (Rollback)", fill=TEXT_SECONDARY, font=FONT_BODY_BOLD)

    # KPIs
    draw_kpi_card(draw, 260, 135, 290, 95, "TWEAK ATTIVI", "9 / 11 attivi", "Ottimizzazioni applicate", ACCENT_CYAN)
    draw_kpi_card(draw, 570, 135, 290, 95, "LIVELLO PRIVACY", "MASSIMA (FORTEZZA)", "Telemetria e annunci bloccati", ACCENT_EMERALD)
    draw_kpi_card(draw, 880, 135, 290, 95, "BLOATWARE RILEVATI", "7 pacchetti AppX", "TikTok, Candy Crush, ecc.", ACCENT_PURPLE)

    # Tweaks Card Table
    draw.rounded_rectangle([260, 245, W - 30, H - 25], radius=10, fill=BG_CARD, outline=BORDER_SUBTLE, width=1)
    draw.text((280, 260), "OTTIMIZZAZIONI PRIVACY, MENU START E SERVIZI IN BACKGROUND", fill=TEXT_PRIMARY, font=FONT_HEADING)

    tweaks = [
        ("Disattiva Telemetria di Sistema (DataCollection)", "Imposta la telemetria di diagnostica Windows al livello minimo (Security/Zero).", True),
        ("Disattiva ID Pubblicitario Utente", "Impedisce a Microsoft e alle app di tracciare le preferenze per annunci mirati.", True),
        ("Disattiva Ricerca Web Bing nel Menu Start", "Rende la ricerca del menu Start istantanea e locale, senza inviare dati online a Bing.", True),
        ("Disattiva App Consigliate e Sponsorizzate nel Menu Start", "Rimuove le app promozionali suggerite automaticamente nella schermata Start.", True),
        ("Disattiva Widget e Notizie MSN sulla Barra Applicazioni", "Rimuove il feed meteo/notizie che consuma memoria RAM e rete in background.", True),
        ("Disattiva Pre-caricamento di Edge in Background (Startup Boost)", "Impedisce a Microsoft Edge di consumare CPU e RAM all'avvio del sistema.", True),
        ("Disattiva Suggerimenti e Quiz nella Schermata di Blocco", "Impedisce a Windows Spotlight di inserire annunci pubblicitari al login.", True),
    ]

    ty = 295
    for name, desc, active in tweaks:
        draw.rounded_rectangle([280, ty, W - 50, ty + 52], radius=6, fill=BG_CARD_SUBTLE, outline=BORDER_SUBTLE, width=1)

        # Toggle Switch Graphic
        sw_bg = ACCENT_EMERALD if active else TEXT_MUTED
        draw.rounded_rectangle([300, ty + 16, 336, ty + 36], radius=10, fill=sw_bg)
        knob_x = 324 if active else 308
        draw.ellipse([knob_x - 6, ty + 18, knob_x + 6, ty + 34], fill=(255, 255, 255))

        draw.text((355, ty + 8), name, fill=TEXT_PRIMARY, font=FONT_BODY_BOLD)
        draw.text((355, ty + 28), desc, fill=TEXT_SECONDARY, font=FONT_SMALL)

        draw_pill(draw, W - 260, ty + 14, "CONSIGLIATO", ACCENT_EMERALD, (16, 185, 129, 30))
        draw_pill(draw, W - 140, ty + 14, "ATTIVO", ACCENT_CYAN, (6, 182, 212, 30))
        ty += 58

    return img

# ----------------------------------------------------------------------------
# 5. HERO BANNER
# ----------------------------------------------------------------------------
def create_hero_banner():
    BW, BH = 1200, 480
    img = Image.new("RGB", (BW, BH), BG_CANVAS)
    draw = ImageDraw.Draw(img)

    # Ambient glow
    for r in range(120, 0, -5):
        draw.ellipse([BW//2 - r*4, -r*2, BW//2 + r*4, r*2], fill=(234, 88, 12, int(8 * (1 - r/120))))

    # Logo & Badge
    draw.rounded_rectangle([BW//2 - 40, 40, BW//2 + 40, 120], radius=16, fill=ACCENT_RUST)
    draw.text((BW//2 - 24, 46), "RC", fill=(255, 255, 255), font=FONT_HERO_TITLE)

    # Title
    draw.text((BW//2 - 160, 140), "RustCleaner", fill=TEXT_PRIMARY, font=FONT_HERO_TITLE)
    draw.text((BW//2 - 320, 205), "Zero-Telemetry, High-Performance Windows Optimizer in Pure Rust", fill=TEXT_SECONDARY, font=FONT_HERO_SUB)

    # Feature Badges
    pills = [
        ("⚡ 100% Pure Rust", ACCENT_RUST),
        ("🛡️ Zero Telemetry", ACCENT_EMERALD),
        ("🧠 0 MB Background RAM", ACCENT_CYAN),
        ("📦 Single Portable EXE (+crt-static)", ACCENT_PURPLE),
        ("🌐 7 Languages", ACCENT_AMBER),
    ]

    total_w = sum(len(text) * 8 + 30 for text, _ in pills)
    cur_x = (BW - total_w) // 2
    for text, col in pills:
        pw = draw_pill(draw, cur_x, 260, text, col, (col[0], col[1], col[2], 30), font=FONT_BODY_BOLD)
        cur_x += pw + 12

    # Highlights Row
    boxes = [
        ("Deep Residual Hunter", "Nukes leftovers from uninstalled apps (Revo killer)."),
        ("SHA-256 Duplicate Finder", "3-stage zero-waste parallel duplicate detector."),
        ("Windows 11 Decrapifier", "Strips Bing search, Cortana & MSN widgets safely."),
        ("Clean Registry (.reg Backup)", "Instant rollback backup before deleting orphaned keys."),
    ]

    bx = 40
    for b_title, b_desc in boxes:
        draw.rounded_rectangle([bx, 320, bx + 265, 430], radius=10, fill=BG_CARD, outline=BORDER_SUBTLE, width=1)
        draw.text((bx + 14, 335), b_title, fill=ACCENT_CYAN, font=FONT_BODY_BOLD)
        draw.text((bx + 14, 365), b_desc, fill=TEXT_SECONDARY, font=FONT_SMALL)
        bx += 285

    return img

if __name__ == "__main__":
    print("Generating assets...")
    banner = create_hero_banner()
    banner.save(os.path.join(ASSETS_DIR, "hero_banner.png"))
    print("-> hero_banner.png")

    c1 = create_cleaner_screenshot()
    c1.save(os.path.join(ASSETS_DIR, "screenshot_cleaner.png"))
    print("-> screenshot_cleaner.png")

    c2 = create_disk_analyzer_screenshot()
    c2.save(os.path.join(ASSETS_DIR, "screenshot_disk_analyzer.png"))
    print("-> screenshot_disk_analyzer.png")

    c3 = create_residual_hunter_screenshot()
    c3.save(os.path.join(ASSETS_DIR, "screenshot_residual_hunter.png"))
    print("-> screenshot_residual_hunter.png")

    c4 = create_decrapifier_screenshot()
    c4.save(os.path.join(ASSETS_DIR, "screenshot_decrapifier.png"))
    print("-> screenshot_decrapifier.png")

    # Generate Animated Demo GIF
    print("Generating animated demo.gif...")
    c1.save(
        os.path.join(ASSETS_DIR, "demo.gif"),
        save_all=True,
        append_images=[c2, c3, c4],
        duration=2200,  # 2.2 seconds per slide
        loop=0
    )
    print("-> demo.gif")
    print("All assets generated successfully in:", ASSETS_DIR)
