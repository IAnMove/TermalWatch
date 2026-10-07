//! ThermalWatch - monitor de temperatura, CPU, RAM y escritura de disco.
//! Ventana normal de Hyprland con la estética de Omarchy: usa la paleta y la
//! fuente del tema activo y se recolorea en caliente al cambiar de tema.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::f64::consts::PI;
use std::fs;
use std::rc::Rc;
use std::time::{Duration, Instant};

use adw::prelude::*;
use gtk::cairo;
use gtk::{gdk, glib, pango};

const APP_ID: &str = "dev.ina.thermalwatch";
const WARM: f64 = 75.0;
const HOT: f64 = 88.0;
const CRIT: f64 = 95.0;
const T_MIN: f64 = 30.0;
const T_MAX: f64 = 105.0;
const HIST: usize = 150;
const MINI: usize = 60;
const PROTECTED: &[&str] = &[
    "Hyprland", "hyprland", "systemd", "(sd-pam)", "quickshell", "pipewire", "wireplumber",
    "dbus-broker", "dbus-broker-lau", "Xwayland", "pipewire-pulse", "gnome-keyring-d",
    "xdg-desktop-por", "hyprlock", "hypridle", "mako", "waybar",
];

type Rgb = (f64, f64, f64);

const CSS: &str = r#"
window.tw { background: @tw_bg; color: @tw_fg; }
window.tw label, window.tw button { font-family: "FONTFAMILY", monospace; }
.title { color: @tw_accent; font-weight: 800; letter-spacing: 3px; font-size: 12px; }
.themename { color: @tw_dfg; font-size: 11px; }
.statline { color: @tw_dfg; font-size: 11px; }
.card { background: @tw_dbg; border: 1px solid @tw_muted; border-radius: 0; padding: 8px 10px; }
.cardlabel { color: @tw_dfg; font-size: 10px; font-weight: 800; letter-spacing: 2px; }
.cardvalue { color: @tw_bfg; font-size: 19px; font-weight: 800; }
.cardsub { color: @tw_dfg; font-size: 10px; }
.section { color: @tw_dfg; font-size: 10px; font-weight: 800; letter-spacing: 2px; }
.procrow { background: @tw_dbg; border: 1px solid @tw_muted; border-radius: 0; padding: 6px 10px; margin: 2px 0; }
.procrow.culprit { border-color: @tw_red; background: alpha(@tw_red, 0.12); }
.procname { color: @tw_bfg; font-weight: 700; }
.procmeta { color: @tw_dfg; font-size: 10px; }
.proccpu { color: @tw_fg; font-size: 11px; }
button.closebtn { background: transparent; color: @tw_red; border: 1px solid @tw_red; border-radius: 0;
                  padding: 2px 10px; min-height: 0; font-weight: 700; box-shadow: none; }
button.closebtn:hover { background: @tw_red; color: @tw_bg; }
.banner { background: @tw_red; border-radius: 0; padding: 8px 12px; }
.banner label { color: @tw_dbg; font-weight: 700; }
button.bannerbtn { background: @tw_dbg; color: @tw_red; border-radius: 0; border: none; font-weight: 800;
                   min-height: 0; padding: 4px 12px; box-shadow: none; }
progressbar trough { min-height: 4px; border-radius: 0; background: @tw_sel; border: none; }
progressbar progress { min-height: 4px; border-radius: 0; background: @tw_accent; border: none; }
.culprit progressbar progress { background: @tw_red; }
.tabs button { background: transparent; color: @tw_dfg; border: 1px solid @tw_muted; border-radius: 0;
               min-height: 0; padding: 2px 12px; font-size: 11px; font-weight: 700; box-shadow: none; }
.tabs button:checked { background: @tw_accent; color: @tw_bg; border-color: @tw_accent; }
scrolledwindow undershoot, scrolledwindow overshoot { background: none; }
"#;

// ---------------------------------------------------------------- i18n

#[derive(Clone, Copy, PartialEq)]
enum Lang {
    En,
    Es,
    Fr,
    De,
    Pt,
    It,
}

/// Idioma activo del SO: THERMALWATCH_LANG, LANGUAGE, LC_ALL, LC_MESSAGES, LANG y /etc/locale.conf.
fn lang() -> Lang {
    static L: std::sync::OnceLock<Lang> = std::sync::OnceLock::new();
    *L.get_or_init(|| {
        let mut cands: Vec<String> = ["THERMALWATCH_LANG", "LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|k| std::env::var(k).ok())
            .filter(|v| !v.is_empty())
            .collect();
        if let Ok(c) = fs::read_to_string("/etc/locale.conf") {
            for l in c.lines() {
                if let Some(v) = l.strip_prefix("LANG=") {
                    cands.push(v.trim().trim_matches('"').to_string());
                }
            }
        }
        for c in cands {
            for part in c.split(':') {
                let code = part.to_lowercase();
                if code == "c" || code.starts_with("c.") || code == "posix" || code.len() < 2 {
                    continue;
                }
                match code.get(..2) {
                    Some("es") => return Lang::Es,
                    Some("fr") => return Lang::Fr,
                    Some("de") => return Lang::De,
                    Some("pt") => return Lang::Pt,
                    Some("it") => return Lang::It,
                    Some("en") => return Lang::En,
                    _ => {}
                }
            }
        }
        Lang::En
    })
}

// clave -> [en, es, fr, de, pt, it]
const STR: &[(&str, [&str; 6])] = &[
    ("crit", ["CRITICAL!", "¡CRÍTICO!", "CRITIQUE !", "KRITISCH!", "CRÍTICO!", "CRITICO!"]),
    ("veryhot", ["VERY HOT", "MUY CALIENTE", "TRÈS CHAUD", "SEHR HEISS", "MUITO QUENTE", "MOLTO CALDO"]),
    ("hot", ["HOT", "CALIENTE", "CHAUD", "HEISS", "QUENTE", "CALDO"]),
    ("warm", ["WARM", "TEMPLADO", "TIÈDE", "WARM", "MORNO", "TIEPIDO"]),
    ("cool", ["COOL", "FRESCO", "FRAIS", "KÜHL", "FRESCO", "FRESCO"]),
    ("fan", ["FAN", "VENT.", "VENT.", "LÜFTER", "VENT.", "VENTOLA"]),
    ("load", ["load {}", "carga {}", "charge {}", "Last {}", "carga {}", "carico {}"]),
    ("ram_of", ["{}% of {}", "{}% de {}", "{}% sur {}", "{}% von {}", "{}% de {}", "{}% di {}"]),
    ("read", ["read {}/s", "lectura {}/s", "lecture {}/s", "Lesen {}/s", "leitura {}/s", "lettura {}/s"]),
    ("disk_w", ["DISK WRITE", "DISCO ESC.", "DISQUE ÉCR.", "DISK SCHR.", "DISCO ESC.", "DISCO SCR."]),
    ("processes", ["PROCESSES", "PROCESOS", "PROCESSUS", "PROZESSE", "PROCESSOS", "PROCESSI"]),
    ("disk_tab", ["DISK", "DISCO", "DISQUE", "DISK", "DISCO", "DISCO"]),
    ("nproc", ["{} process(es)", "{} proceso(s)", "{} processus", "{} Prozess(e)", "{} processo(s)", "{} processo/i"]),
    ("close", ["Close", "Cerrar", "Fermer", "Schließen", "Fechar", "Chiudi"]),
    ("close_caps", ["CLOSE", "CERRAR", "FERMER", "SCHLIESSEN", "FECHAR", "CHIUDI"]),
    ("overheat", ["OVERHEATING", "SOBRECALENTAMIENTO", "SURCHAUFFE", "ÜBERHITZUNG", "SUPERAQUECIMENTO", "SURRISCALDAMENTO"]),
    ("cause", ["Likely cause", "Causa probable", "Cause probable", "Wahrscheinliche Ursache", "Causa provável", "Causa probabile"]),
    ("dlg_head", ["Close this program?", "¿Desea cerrar el programa?", "Fermer ce programme ?", "Programm beenden?", "Deseja fechar o programa?", "Chiudere il programma?"]),
    ("dlg_body", [
        "{}\n{} process(es) · {}% CPU · {}\n\nA close request will be sent; if it doesn't respond within 3 s, it will be forced.",
        "{}\n{} proceso(s) · {}% CPU · {}\n\nSe enviará una orden de cierre y, si no responde en 3 s, se forzará.",
        "{}\n{} processus · {}% CPU · {}\n\nUne demande de fermeture sera envoyée ; sans réponse sous 3 s, l'arrêt sera forcé.",
        "{}\n{} Prozess(e) · {}% CPU · {}\n\nEs wird eine Beenden-Anfrage gesendet; ohne Antwort innerhalb von 3 s wird erzwungen.",
        "{}\n{} processo(s) · {}% CPU · {}\n\nSerá enviado um pedido de encerramento e, se não responder em 3 s, será forçado.",
        "{}\n{} processo/i · {}% CPU · {}\n\nVerrà inviata una richiesta di chiusura e, se non risponde entro 3 s, verrà forzata.",
    ]),
    ("cancel", ["Cancel", "Cancelar", "Annuler", "Abbrechen", "Cancelar", "Annulla"]),
    ("kill_prog", ["Close program", "Cerrar programa", "Fermer le programme", "Programm beenden", "Fechar programa", "Chiudi programma"]),
    ("closing", ["Closing {}…", "Cerrando {}…", "Fermeture de {}…", "{} wird beendet…", "Fechando {}…", "Chiusura di {}…"]),
    ("panic_title", ["KILL · EXTREME SITUATION", "KILL · SITUACIÓN EXTREMA", "KILL · SITUATION EXTRÊME", "KILL · NOTFALL", "KILL · SITUAÇÃO EXTREMA", "KILL · SITUAZIONE ESTREMA"]),
    ("measuring", ["measuring…", "midiendo…", "mesure…", "messe…", "medindo…", "misurazione…"]),
    ("nothing", ["— nothing relevant —", "— nada relevante —", "— rien de notable —", "— nichts Relevantes —", "— nada relevante —", "— niente di rilevante —"]),
    ("press", ["press {}", "pulsa {}", "appuie {}", "drücke {}", "pressione {}", "premi {}"]),
    ("panic_hint", [
        "press Esc to exit · click outside to close · kills instantly, no confirmation",
        "pulsa Esc para salir · clic fuera para cerrar · mata al instante, sin confirmar",
        "Échap pour quitter · clic à l'extérieur pour fermer · tue instantanément, sans confirmation",
        "Esc zum Beenden · Klick außerhalb schließt · beendet sofort, ohne Rückfrage",
        "Esc para sair · clique fora para fechar · mata na hora, sem confirmação",
        "Esc per uscire · clic fuori per chiudere · termina subito, senza conferma",
    ]),
    ("killed", ["✔ {} killed ({} process(es))", "✔ {} eliminado ({} proceso(s))", "✔ {} tué ({} processus)", "✔ {} beendet ({} Prozess(e))", "✔ {} encerrado ({} processo(s))", "✔ {} terminato ({} processo/i)"]),
];

fn tr(k: &str) -> &'static str {
    let i = match lang() {
        Lang::En => 0,
        Lang::Es => 1,
        Lang::Fr => 2,
        Lang::De => 3,
        Lang::Pt => 4,
        Lang::It => 5,
    };
    STR.iter().find(|(key, _)| *key == k).map_or("?", |(_, v)| v[i])
}

/// Traduce y sustituye cada `{}` por el siguiente argumento.
fn trf(k: &str, args: &[String]) -> String {
    let mut s = tr(k).to_string();
    for a in args {
        if let Some(i) = s.find("{}") {
            s.replace_range(i..i + 2, a);
        }
    }
    s
}

// ---------------------------------------------------------------- paleta

fn parse_hex(s: &str) -> Rgb {
    let s = s.trim().trim_start_matches('#');
    if s.len() < 6 {
        return (1.0, 1.0, 1.0);
    }
    let c = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).unwrap_or(255) as f64 / 255.0;
    (c(0), c(2), c(4))
}

#[derive(Clone)]
struct Palette {
    raw: String,
    name: String,
    dark: bool,
    hex: HashMap<String, String>,
    fg: Rgb,
    dfg: Rgb,
    muted: Rgb,
    accent: Rgb,
    red: Rgb,
    yellow: Rgb,
    orange: Rgb,
    green: Rgb,
    cyan: Rgb,
    magenta: Rgb,
    stops: [(f64, Rgb); 5],
}

impl Palette {
    fn load() -> Self {
        let home = std::env::var("HOME").unwrap_or_default();
        let base = format!("{home}/.local/state/omarchy/current");
        let txt = fs::read_to_string(format!("{base}/theme/colors.toml")).unwrap_or_default();
        let name = fs::read_to_string(format!("{base}/theme.name")).unwrap_or_default().trim().to_string();
        let mut m: HashMap<String, String> = [
            ("background", "#1a1b26"), ("dark_background", "#13141c"), ("selection", "#292e42"),
            ("muted", "#414868"), ("accent", "#7aa2f7"), ("foreground", "#a9b1d6"),
            ("dark_foreground", "#565f89"), ("bright_foreground", "#c0caf5"), ("red", "#f7768e"),
            ("yellow", "#e0af68"), ("orange", "#eb927b"), ("green", "#9ece6a"), ("cyan", "#449dab"),
            ("blue", "#7aa2f7"), ("magenta", "#ad8ee6"), ("mode", "dark"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        for line in txt.lines() {
            if let Some((k, v)) = line.split_once('=') {
                m.insert(k.trim().to_string(), v.trim().trim_matches('"').to_string());
            }
        }
        let c = |k: &str| parse_hex(m.get(k).map_or("#ffffff", |s| s.as_str()));
        let (blue, green, yellow, orange, red) = (c("blue"), c("green"), c("yellow"), c("orange"), c("red"));
        Palette {
            raw: format!("{txt}{name}"),
            name,
            dark: m.get("mode").map_or(true, |s| s != "light"),
            fg: c("foreground"),
            dfg: c("dark_foreground"),
            muted: c("muted"),
            accent: c("accent"),
            red,
            yellow,
            orange,
            green,
            cyan: c("cyan"),
            magenta: c("magenta"),
            stops: [(40.0, blue), (58.0, green), (74.0, yellow), (88.0, orange), (100.0, red)],
            hex: m,
        }
    }

    fn css(&self, font: &str) -> String {
        let mut out = String::new();
        for (n, k) in [
            ("tw_bg", "background"), ("tw_dbg", "dark_background"), ("tw_fg", "foreground"),
            ("tw_dfg", "dark_foreground"), ("tw_bfg", "bright_foreground"), ("tw_muted", "muted"),
            ("tw_sel", "selection"), ("tw_accent", "accent"), ("tw_red", "red"), ("tw_green", "green"),
        ] {
            out += &format!("@define-color {n} {};\n", self.hex.get(k).cloned().unwrap_or_default());
        }
        out + &CSS.replace("FONTFAMILY", font)
    }

    fn heat(&self, t: f64) -> Rgb {
        let s = &self.stops;
        if t <= s[0].0 {
            return s[0].1;
        }
        for w in s.windows(2) {
            let ((t0, c0), (t1, c1)) = (w[0], w[1]);
            if t <= t1 {
                let k = (t - t0) / (t1 - t0);
                return (c0.0 + (c1.0 - c0.0) * k, c0.1 + (c1.1 - c0.1) * k, c0.2 + (c1.2 - c0.2) * k);
            }
        }
        s[4].1
    }
}

fn current_font() -> String {
    std::process::Command::new("omarchy")
        .args(["font", "current"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "JetBrainsMono Nerd Font".into())
}

// ---------------------------------------------------------------- sensores

fn rd_int(path: &str) -> Option<i64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

#[derive(Default, Clone)]
struct Reading {
    cpu: Option<f64>,
    gpu: Option<f64>,
    nvme: Option<f64>,
    fan: Option<i64>,
    load: f64,
}

struct Sensors {
    by_name: HashMap<String, Vec<String>>,
}

impl Sensors {
    fn new() -> Self {
        let mut by_name: HashMap<String, Vec<String>> = HashMap::new();
        if let Ok(rd) = fs::read_dir("/sys/class/hwmon") {
            for e in rd.flatten() {
                let p = e.path();
                if let Ok(n) = fs::read_to_string(p.join("name")) {
                    by_name.entry(n.trim().to_string()).or_default().push(p.to_string_lossy().into_owned());
                }
            }
        }
        Sensors { by_name }
    }

    fn temp(&self, name: &str) -> Option<f64> {
        self.by_name
            .get(name)?
            .iter()
            .find_map(|p| rd_int(&format!("{p}/temp1_input")))
            .map(|v| v as f64 / 1000.0)
    }

    fn read(&self) -> Reading {
        let cpu = self.temp("k10temp").or_else(|| self.temp("coretemp")).or_else(|| self.temp("acpitz"));
        let mut fan: Option<i64> = None;
        for name in ["hp", "thinkpad"] {
            for p in self.by_name.get(name).into_iter().flatten() {
                for i in 1..=8 {
                    if let Some(v) = rd_int(&format!("{p}/fan{i}_input")) {
                        fan = Some(fan.map_or(v, |f| f.max(v)));
                    }
                }
            }
        }
        let load = fs::read_to_string("/proc/loadavg")
            .ok()
            .and_then(|s| s.split_whitespace().next().and_then(|x| x.parse().ok()))
            .unwrap_or(0.0);
        Reading { cpu, gpu: self.temp("amdgpu"), nvme: self.temp("nvme"), fan, load }
    }
}

/// CPU total, RAM y disco (lectura/escritura) del sistema.
#[derive(Default, Clone)]
struct SysReading {
    cpu_pct: f64,
    ram_used: f64,
    ram_total: f64,
    disk_w: f64,
    disk_r: f64,
}

#[derive(Default)]
struct SysSampler {
    prev_cpu: Option<(u64, u64)>,
    prev_disk: Option<(u64, u64, Instant)>,
}

fn is_whole_disk(n: &str) -> bool {
    if n.starts_with("nvme") || n.starts_with("mmcblk") {
        !n[4..].contains('p')
    } else if n.starts_with("sd") || n.starts_with("vd") || n.starts_with("hd") || n.starts_with("xvd") {
        !n.chars().last().map_or(true, |c| c.is_ascii_digit())
    } else {
        false
    }
}

impl SysSampler {
    fn sample(&mut self) -> SysReading {
        let mut r = SysReading::default();
        if let Ok(s) = fs::read_to_string("/proc/stat") {
            if let Some(l) = s.lines().next() {
                let v: Vec<u64> = l.split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
                if v.len() >= 8 {
                    let idle = v[3] + v[4];
                    let total: u64 = v[..8].iter().sum();
                    if let Some((pi, pt)) = self.prev_cpu {
                        let dt = total.saturating_sub(pt) as f64;
                        if dt > 0.0 {
                            r.cpu_pct = (1.0 - idle.saturating_sub(pi) as f64 / dt) * 100.0;
                        }
                    }
                    self.prev_cpu = Some((idle, total));
                }
            }
        }
        if let Ok(s) = fs::read_to_string("/proc/meminfo") {
            let (mut total, mut avail) = (0.0, 0.0);
            for l in s.lines() {
                let mut it = l.split_whitespace();
                match it.next() {
                    Some("MemTotal:") => total = it.next().and_then(|x| x.parse().ok()).unwrap_or(0.0),
                    Some("MemAvailable:") => avail = it.next().and_then(|x| x.parse().ok()).unwrap_or(0.0),
                    _ => {}
                }
            }
            r.ram_total = total * 1024.0;
            r.ram_used = (total - avail) * 1024.0;
        }
        if let Ok(s) = fs::read_to_string("/proc/diskstats") {
            let (mut w, mut rd) = (0u64, 0u64);
            for l in s.lines() {
                let p: Vec<&str> = l.split_whitespace().collect();
                if p.len() > 9 && is_whole_disk(p[2]) {
                    rd += p[5].parse::<u64>().unwrap_or(0) * 512;
                    w += p[9].parse::<u64>().unwrap_or(0) * 512;
                }
            }
            let now = Instant::now();
            if let Some((pr, pw, pt)) = self.prev_disk {
                let dt = now.duration_since(pt).as_secs_f64();
                if dt > 0.0 {
                    r.disk_w = w.saturating_sub(pw) as f64 / dt;
                    r.disk_r = rd.saturating_sub(pr) as f64 / dt;
                }
            }
            self.prev_disk = Some((rd, w, now));
        }
        r
    }
}

// ---------------------------------------------------------------- procesos

#[derive(Clone)]
struct Group {
    key: String,
    pids: Vec<i32>,
    cpu: f64,
    mem: f64,
    wbps: f64,
    protected: bool,
}

struct ProcSampler {
    prev: HashMap<i32, (u64, u64)>,
    prev_t: Option<Instant>,
    clk: f64,
    page: f64,
    uid: u32,
    self_pid: i32,
}

fn read_write_bytes(pid: i32) -> u64 {
    fs::read_to_string(format!("/proc/{pid}/io"))
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("write_bytes:").and_then(|v| v.trim().parse().ok()))
        })
        .unwrap_or(0)
}

impl ProcSampler {
    fn new() -> Self {
        ProcSampler {
            prev: HashMap::new(),
            prev_t: None,
            clk: unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as f64,
            page: unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as f64,
            uid: unsafe { libc::getuid() },
            self_pid: std::process::id() as i32,
        }
    }

    fn key(comm: &str, pid: i32) -> String {
        if comm.starts_with("chrom") || comm.starts_with("brave") {
            if let Ok(c) = fs::read(format!("/proc/{pid}/cmdline")) {
                if c.windows(8).any(|w| w == b"headless") {
                    return "chromium headless".into();
                }
            }
        }
        comm.to_string()
    }

    fn sample(&mut self) -> Vec<Group> {
        use std::os::unix::fs::MetadataExt;
        let now = Instant::now();
        let dt = self.prev_t.map(|t| now.duration_since(t).as_secs_f64());
        let mut cur = HashMap::new();
        let mut groups: HashMap<String, Group> = HashMap::new();
        if let Ok(rd) = fs::read_dir("/proc") {
            for e in rd.flatten() {
                let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<i32>().ok()) else { continue };
                if pid == self.self_pid {
                    continue;
                }
                let Ok(md) = fs::metadata(format!("/proc/{pid}")) else { continue };
                if md.uid() != self.uid {
                    continue;
                }
                let Ok(s) = fs::read_to_string(format!("/proc/{pid}/stat")) else { continue };
                let (Some(a), Some(b)) = (s.find('('), s.rfind(')')) else { continue };
                let comm = &s[a + 1..b];
                let rest: Vec<&str> = s[b + 2..].split_whitespace().collect();
                let (Some(u), Some(k), Some(rss)) = (rest.get(11), rest.get(12), rest.get(21)) else { continue };
                let (Ok(u), Ok(k), Ok(rss)) = (u.parse::<u64>(), k.parse::<u64>(), rss.parse::<f64>()) else { continue };
                let ticks = u + k;
                let key = Self::key(comm, pid);
                let prev_tick = self.prev.get(&pid).map(|(t, _)| *t).unwrap_or(ticks);
                // Only inspect disk I/O if the process actually ran or has substantial memory
                let wb = if ticks > prev_tick || rss > 2500.0 {
                    read_write_bytes(pid)
                } else {
                    self.prev.get(&pid).map(|(_, w)| *w).unwrap_or(0)
                };
                cur.insert(pid, (ticks, wb));
                let g = groups.entry(key.clone()).or_insert_with(|| Group {
                    protected: PROTECTED.contains(&comm),
                    key,
                    pids: vec![],
                    cpu: 0.0,
                    mem: 0.0,
                    wbps: 0.0,
                });
                g.pids.push(pid);
                g.mem += rss.max(0.0) * self.page;
                if let (Some(dt), Some((pt, pw))) = (dt, self.prev.get(&pid)) {
                    if dt > 0.0 {
                        g.cpu += ticks.saturating_sub(*pt) as f64 / self.clk / dt * 100.0;
                        g.wbps += wb.saturating_sub(*pw) as f64 / dt;
                    }
                }
            }
        }
        self.prev = cur;
        self.prev_t = Some(now);
        let mut out: Vec<Group> = groups.into_values().collect();
        out.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
        out
    }
}

fn fmt_bytes(b: f64) -> String {
    if b >= 1e9 {
        format!("{:.1} GB", b / 1e9)
    } else if b >= 1e6 {
        format!("{:.0} MB", b / 1e6)
    } else {
        format!("{:.0} KB", b / 1e3)
    }
}

// ---------------------------------------------------------------- estado

struct Particle {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    life: f64,
    size: f64,
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.next()
    }
}

struct RowW {
    row: gtk::Box,
    stats: gtk::Label,
    bar: gtk::ProgressBar,
    meta: gtk::Label,
}

struct State {
    pal: Palette,
    font: String,
    target: f64,
    shown: f64,
    temp_hist: VecDeque<f64>,
    cpu_hist: VecDeque<f64>,
    ram_hist: VecDeque<f64>,
    disk_hist: VecDeque<f64>,
    groups: Vec<Group>,
    ram_total: f64,
    sort: usize,
    rows: HashMap<String, RowW>,
    row_keys: Vec<String>,
    parts: Vec<Particle>,
    rings: Vec<f64>,
    phase: f64,
    last_tick: Option<f64>,
    ring_clock: f64,
    last_draw: Option<f64>,
    animating: bool,
    rng: Rng,
    sensors: Sensors,
    sys: SysSampler,
    sampler: ProcSampler,
    culprit: Option<String>,
}

struct Ui {
    st: Rc<RefCell<State>>,
    win: adw::ApplicationWindow,
    provider: gtk::CssProvider,
    toasts: adw::ToastOverlay,
    revealer: gtk::Revealer,
    banner_lbl: gtk::Label,
    theme_lbl: gtk::Label,
    stat_lbl: gtk::Label,
    hero: gtk::DrawingArea,
    spark: gtk::DrawingArea,
    minis: Vec<gtk::DrawingArea>,
    plist: gtk::Box,
    v_cpu: gtk::Label,
    s_cpu: gtk::Label,
    v_ram: gtk::Label,
    s_ram: gtk::Label,
    v_disk: gtk::Label,
    s_disk: gtk::Label,
}

fn intensity(shown: f64) -> f64 {
    if shown < HOT { 0.0 } else { ((shown - HOT) / (T_MAX - HOT)).clamp(0.0, 1.0) }
}

fn push_cap(d: &mut VecDeque<f64>, v: f64, cap: usize) {
    if d.len() == cap {
        d.pop_front();
    }
    d.push_back(v);
}

// ---------------------------------------------------------------- dibujo

fn draw_hero(st: &State, cr: &cairo::Context, w: i32, h: i32) {
    let p = &st.pal;
    let (w, h) = (w as f64, h as f64);
    let t = st.shown;
    let inten = intensity(t);
    let (cx, cy) = (w / 2.0, h / 2.0 + 4.0);
    let r = (w * 0.5).min(h * 0.5) * 0.8;
    let col = p.heat(t);
    let pulse = 0.5 + 0.5 * (st.phase * (3.0 + 6.0 * inten)).sin();

    if inten > 0.0 {
        let g = cairo::RadialGradient::new(cx, cy, r * 0.4, cx, cy, r * 1.3);
        g.add_color_stop_rgba(0.0, p.red.0, p.red.1, p.red.2, 0.04 + 0.18 * inten * pulse);
        g.add_color_stop_rgba(1.0, p.red.0, p.red.1, p.red.2, 0.0);
        let _ = cr.set_source(&g);
        cr.arc(cx, cy, r * 1.3, 0.0, 2.0 * PI);
        let _ = cr.fill();
        for rg in &st.rings {
            cr.set_line_width(3.0 * (1.0 - rg));
            cr.set_source_rgba(p.red.0, p.red.1, p.red.2, 0.6 * (1.0 - rg) * inten);
            cr.arc(cx, cy, r * (1.0 + rg * 0.55), 0.0, 2.0 * PI);
            let _ = cr.stroke();
        }
    }

    let (a0, a1) = (135f64.to_radians(), 405f64.to_radians());
    cr.set_line_cap(cairo::LineCap::Butt);
    cr.set_line_width(12.0);
    cr.set_source_rgba(p.muted.0, p.muted.1, p.muted.2, 0.55);
    cr.arc(cx, cy, r, a0, a1);
    let _ = cr.stroke();

    // marcas de escala
    for i in 0..=15 {
        let f = i as f64 / 15.0;
        let a = a0 + (a1 - a0) * f;
        let (r0, r1) = (r - 15.0, r - if i % 5 == 0 { 24.0 } else { 20.0 });
        cr.set_line_width(1.5);
        cr.set_source_rgba(p.dfg.0, p.dfg.1, p.dfg.2, 0.8);
        cr.move_to(cx + r0 * a.cos(), cy + r0 * a.sin());
        cr.line_to(cx + r1 * a.cos(), cy + r1 * a.sin());
        let _ = cr.stroke();
    }

    let frac = ((t - T_MIN) / (T_MAX - T_MIN)).clamp(0.0, 1.0);
    let steps = ((18.0 * frac) as i32).max(1);
    for i in 0..steps {
        let (f0, f1) = (i as f64 / 18.0, (i + 1) as f64 / 18.0);
        let c = p.heat(T_MIN + (T_MAX - T_MIN) * f0);
        cr.set_source_rgb(c.0, c.1, c.2);
        cr.set_line_width(12.0);
        cr.arc(cx, cy, r, a0 + (a1 - a0) * f0, a0 + (a1 - a0) * (f1 + 0.004).min(frac));
        let _ = cr.stroke();
    }

    let ang = a0 + (a1 - a0) * frac;
    cr.set_source_rgb(p.fg.0, p.fg.1, p.fg.2);
    cr.arc(cx + r * ang.cos(), cy + r * ang.sin(), 4.5 + 2.0 * inten * pulse, 0.0, 2.0 * PI);
    let _ = cr.fill();

    for q in &st.parts {
        let px = cx + q.x * r * 0.9;
        let py = cy + r * 0.85 - q.y * r * 1.7;
        let k = q.life.clamp(0.0, 1.0);
        let c = (
            p.orange.0 * k + p.yellow.0 * (1.0 - k),
            p.orange.1 * k + p.yellow.1 * (1.0 - k),
            p.orange.2 * k + p.yellow.2 * (1.0 - k),
        );
        cr.set_source_rgba(c.0, c.1, c.2, k * 0.9);
        cr.rectangle(px, py, q.size * k + 1.0, q.size * k + 1.0);
        let _ = cr.fill();
    }

    let (mut sx, mut sy) = (0.0, 0.0);
    if inten > 0.3 {
        sx = (st.phase * 55.0).sin() * 2.2 * inten;
        sy = (st.phase * 47.0).cos() * 1.6 * inten;
    }
    cr.select_font_face(&st.font, cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    cr.set_font_size(r * 0.55);
    let txt = format!("{:.0}°", t);
    if let Ok(e) = cr.text_extents(&txt) {
        cr.set_source_rgb(col.0, col.1, col.2);
        cr.move_to(cx - e.width() / 2.0 - e.x_bearing() + sx, cy + e.height() / 2.0 - 2.0 + sy);
        let _ = cr.show_text(&txt);
    }

    let (label, lc) = if t >= CRIT {
        (tr("crit"), p.red)
    } else if t >= HOT {
        (tr("veryhot"), p.orange)
    } else if t >= WARM {
        (tr("hot"), p.yellow)
    } else if t >= 55.0 {
        (tr("warm"), p.green)
    } else {
        (tr("cool"), p.cyan)
    };
    cr.set_font_size(r * 0.17);
    if let Ok(e) = cr.text_extents(label) {
        let alpha = if t < HOT { 1.0 } else { 0.55 + 0.45 * pulse };
        cr.set_source_rgba(lc.0, lc.1, lc.2, alpha);
        cr.move_to(cx - e.width() / 2.0 - e.x_bearing(), cy + r * 0.5);
        let _ = cr.show_text(label);
    }

    cr.select_font_face(&st.font, cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    cr.set_font_size(r * 0.12);
    let sub = "CPU · Tctl";
    if let Ok(e) = cr.text_extents(sub) {
        cr.set_source_rgba(p.dfg.0, p.dfg.1, p.dfg.2, 1.0);
        cr.move_to(cx - e.width() / 2.0 - e.x_bearing(), cy - r * 0.42);
        let _ = cr.show_text(sub);
    }
}

fn draw_series(
    cr: &cairo::Context,
    w: i32,
    h: i32,
    data: &VecDeque<f64>,
    cap: usize,
    (lo, hi): (f64, f64),
    col: Rgb,
    thr: Option<(f64, Rgb)>,
) {
    let (w, h) = (w as f64, h as f64);
    let data: Vec<f64> = data.iter().copied().collect();
    if data.len() < 2 {
        return;
    }
    let step = w / (cap as f64 - 1.0);
    let off = w - (data.len() as f64 - 1.0) * step;
    let xs = |i: usize| i as f64 * step + off;
    let ys = |v: f64| h - 2.0 - (v.clamp(lo, hi) - lo) / (hi - lo).max(1e-9) * (h - 4.0);

    if let Some((tv, tc)) = thr {
        cr.set_source_rgba(tc.0, tc.1, tc.2, 0.4);
        cr.set_line_width(1.0);
        cr.set_dash(&[4.0, 4.0], 0.0);
        cr.move_to(0.0, ys(tv));
        cr.line_to(w, ys(tv));
        let _ = cr.stroke();
        cr.set_dash(&[], 0.0);
    }
    let g = cairo::LinearGradient::new(0.0, 0.0, 0.0, h);
    g.add_color_stop_rgba(0.0, col.0, col.1, col.2, 0.35);
    g.add_color_stop_rgba(1.0, col.0, col.1, col.2, 0.0);
    cr.move_to(xs(0), h);
    for (i, v) in data.iter().enumerate() {
        cr.line_to(xs(i), ys(*v));
    }
    cr.line_to(xs(data.len() - 1), h);
    cr.close_path();
    let _ = cr.set_source(&g);
    let _ = cr.fill();
    cr.set_line_width(1.8);
    cr.set_line_join(cairo::LineJoin::Miter);
    cr.set_source_rgb(col.0, col.1, col.2);
    cr.move_to(xs(0), ys(data[0]));
    for (i, v) in data.iter().enumerate() {
        cr.line_to(xs(i), ys(*v));
    }
    let _ = cr.stroke();
}

// ---------------------------------------------------------------- lógica UI

fn update_sensors(ui: &Rc<Ui>) {
    let (mut d, s) = {
        let mut st = ui.st.borrow_mut();
        let d = st.sensors.read();
        let s = st.sys.sample();
        (d, s)
    };
    if let Some(v) = std::env::var("THERMALWATCH_DEMO").ok().and_then(|s| s.parse::<f64>().ok()) {
        d.cpu = Some(v);
    }
    {
        let mut st = ui.st.borrow_mut();
        if let Some(c) = d.cpu {
            st.target = c;
            push_cap(&mut st.temp_hist, c, HIST);
        }
        st.ram_total = s.ram_total;
        push_cap(&mut st.cpu_hist, s.cpu_pct, MINI);
        push_cap(&mut st.ram_hist, s.ram_used, MINI);
        push_cap(&mut st.disk_hist, s.disk_w, MINI);
    }
    let f = |v: Option<f64>| v.map_or("--".to_string(), |v| format!("{v:.0}°"));
    ui.stat_lbl.set_text(&format!(
        "GPU {}  ·  NVMe {}  ·  {} {}",
        f(d.gpu),
        f(d.nvme),
        tr("fan"),
        d.fan.map_or("--".to_string(), |v| format!("{v} rpm"))
    ));
    ui.v_cpu.set_text(&format!("{:.0}%", s.cpu_pct));
    ui.s_cpu.set_text(&trf("load", &[format!("{:.1}", d.load)]));
    ui.v_ram.set_text(&fmt_bytes(s.ram_used));
    ui.s_ram.set_text(&trf(
        "ram_of",
        &[format!("{:.0}", s.ram_used / s.ram_total.max(1.0) * 100.0), fmt_bytes(s.ram_total)],
    ));
    ui.v_disk.set_text(&format!("{}/s", fmt_bytes(s.disk_w)));
    ui.s_disk.set_text(&trf("read", &[fmt_bytes(s.disk_r)]));
    ui.spark.queue_draw();
    ensure_animating(ui);
    for m in &ui.minis {
        m.queue_draw();
    }
    refresh_banner(ui);
}

fn sample_procs(ui: &Rc<Ui>) {
    {
        let mut st = ui.st.borrow_mut();
        let g = st.sampler.sample();
        st.groups = g;
    }
    render_procs(ui);
}

fn metric(g: &Group, sort: usize) -> f64 {
    match sort {
        0 => g.cpu,
        1 => g.mem,
        _ => g.wbps,
    }
}

fn render_procs(ui: &Rc<Ui>) {
    let (mut all, sort, ram_total) = {
        let st = ui.st.borrow();
        (st.groups.clone(), st.sort, st.ram_total)
    };
    all.sort_by(|a, b| metric(b, sort).partial_cmp(&metric(a, sort)).unwrap_or(std::cmp::Ordering::Equal));
    let min = [1.0, 0.0, 1024.0][sort];
    let mut top: Vec<Group> = all.iter().filter(|g| metric(g, sort) > min).take(6).cloned().collect();
    if top.is_empty() {
        top = all.iter().take(3).cloned().collect();
    }
    let mut keys: Vec<String> = top.iter().map(|g| g.key.clone()).collect();
    keys.sort();
    let rebuild = ui.st.borrow().row_keys != keys;
    if rebuild {
        while let Some(c) = ui.plist.first_child() {
            ui.plist.remove(&c);
        }
        let mut rows = HashMap::new();
        for g in &top {
            rows.insert(g.key.clone(), build_row(ui, g));
        }
        let mut st = ui.st.borrow_mut();
        st.rows = rows;
        st.row_keys = keys;
    }
    let st = ui.st.borrow();
    let ncpu = std::thread::available_parallelism().map_or(1, |n| n.get()) as f64;
    let max_w = top.iter().map(|g| g.wbps).fold(1.0, f64::max);
    let mut prev: Option<gtk::Widget> = None;
    let culprit = st
        .groups
        .iter()
        .find(|g| !g.protected)
        .filter(|g| st.shown >= HOT && g.cpu >= 40.0)
        .map(|g| g.key.clone());
    for g in &top {
        if let Some(r) = st.rows.get(&g.key) {
            r.stats.set_text(&format!("{:>4.0}%  {:>7}  {:>8}/s", g.cpu, fmt_bytes(g.mem), fmt_bytes(g.wbps)));
            r.meta.set_text(&trf("nproc", &[g.pids.len().to_string()]));
            r.bar.set_fraction(
                match sort {
                    0 => g.cpu / (ncpu * 100.0),
                    1 => g.mem / ram_total.max(1.0),
                    _ => g.wbps / max_w,
                }
                .clamp(0.0, 1.0),
            );
            if culprit.as_deref() == Some(g.key.as_str()) {
                r.row.add_css_class("culprit");
            } else {
                r.row.remove_css_class("culprit");
            }
            ui.plist.reorder_child_after(&r.row, prev.as_ref());
            prev = Some(r.row.clone().upcast());
        }
    }
    drop(st);
    refresh_banner(ui);
}

fn build_row(ui: &Rc<Ui>, g: &Group) -> RowW {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("procrow");
    let left = gtk::Box::new(gtk::Orientation::Vertical, 3);
    left.set_hexpand(true);
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name = gtk::Label::builder()
        .label(g.key.as_str())
        .xalign(0.0)
        .hexpand(true)
        .ellipsize(pango::EllipsizeMode::End)
        .build();
    name.add_css_class("procname");
    let stats = gtk::Label::builder().label("").xalign(1.0).build();
    stats.add_css_class("proccpu");
    top.append(&name);
    top.append(&stats);
    let bar = gtk::ProgressBar::new();
    let meta = gtk::Label::builder().xalign(0.0).build();
    meta.add_css_class("procmeta");
    left.append(&top);
    left.append(&bar);
    left.append(&meta);
    row.append(&left);
    if !g.protected {
        let btn = gtk::Button::builder().label(tr("close")).valign(gtk::Align::Center).build();
        btn.add_css_class("closebtn");
        let (ui2, key) = (ui.clone(), g.key.clone());
        btn.connect_clicked(move |_| confirm_kill(&ui2, &key));
        row.append(&btn);
    }
    ui.plist.append(&row);
    RowW { row, stats, bar, meta }
}

fn refresh_banner(ui: &Rc<Ui>) {
    let (show, text, key) = {
        let st = ui.st.borrow();
        match st.groups.iter().find(|g| !g.protected) {
            Some(g) if st.shown >= HOT && g.cpu >= 40.0 => (
                true,
                format!(
                    "<b>🔥 {}</b>\n{}: <b>{}</b> · {:.0}% CPU",
                    tr("overheat"),
                    tr("cause"),
                    glib::markup_escape_text(&g.key),
                    g.cpu
                ),
                Some(g.key.clone()),
            ),
            _ => (false, String::new(), None),
        }
    };
    ui.st.borrow_mut().culprit = key;
    if show {
        ui.banner_lbl.set_markup(&text);
    }
    ui.revealer.set_reveal_child(show);
}

fn confirm_kill(ui: &Rc<Ui>, key: &str) {
    let Some(g) = ui.st.borrow().groups.iter().find(|g| g.key == key).cloned() else { return };
    let d = adw::AlertDialog::new(
        Some(tr("dlg_head")),
        Some(&trf(
            "dlg_body",
            &[g.key.clone(), g.pids.len().to_string(), format!("{:.0}", g.cpu), fmt_bytes(g.mem)],
        )),
    );
    d.add_response("cancel", tr("cancel"));
    d.add_response("kill", tr("kill_prog"));
    d.set_response_appearance("kill", adw::ResponseAppearance::Destructive);
    d.set_default_response(Some("cancel"));
    d.set_close_response("cancel");
    let ui2 = ui.clone();
    d.choose(Some(&ui.win), gtk::gio::Cancellable::NONE, move |resp| {
        if resp == "kill" {
            kill_group(&ui2, &g);
        }
    });
}

fn comm_of(pid: i32) -> Option<String> {
    fs::read_to_string(format!("/proc/{pid}/comm")).ok()
}

fn kill_group(ui: &Rc<Ui>, g: &Group) {
    let mut comms = vec![];
    for &p in &g.pids {
        if let Some(c) = comm_of(p) {
            comms.push((p, c));
            unsafe { libc::kill(p, libc::SIGTERM) };
        }
    }
    let ui2 = ui.clone();
    glib::timeout_add_local_once(Duration::from_secs(3), move || {
        for (p, c) in &comms {
            if comm_of(*p).as_ref() == Some(c) {
                unsafe { libc::kill(*p, libc::SIGKILL) };
            }
        }
        let ui3 = ui2.clone();
        glib::timeout_add_local_once(Duration::from_millis(500), move || sample_procs(&ui3));
    });
    ui.toasts.add_toast(adw::Toast::new(&trf("closing", &[g.key.clone()])));
}


fn ensure_animating(ui: &Rc<Ui>) {
    let mut st = ui.st.borrow_mut();
    if st.animating {
        return;
    }
    st.animating = true;
    drop(st);

    let u = ui.clone();
    glib::timeout_add_local(Duration::from_millis(33), move || {
        let now = glib::monotonic_time() as f64 / 1e6;
        let mut s = u.st.borrow_mut();
        let cont = on_tick(&mut s, now);
        if cont {
            u.hero.queue_draw();
            glib::ControlFlow::Continue
        } else {
            s.animating = false;
            u.hero.queue_draw();
            glib::ControlFlow::Break
        }
    });
}

fn on_tick(st: &mut State, t: f64) -> bool {
    let diff = st.target - st.shown;
    let inten = intensity(st.shown);
    if diff.abs() < 0.6 && inten == 0.0 && st.parts.is_empty() && st.rings.is_empty() {
        st.shown = st.target;
        return false;
    }
    let animating = inten > 0.0 || !st.parts.is_empty() || !st.rings.is_empty() || diff.abs() >= 0.6;

    if !animating {
        st.shown = st.target;
        st.last_tick = Some(t);
        return false;
    }

    // Limit animation frame rate to 30 FPS to save CPU (especially on high-refresh 144Hz displays)
    if let Some(ld) = st.last_draw {
        if t - ld < 0.033 {
            return false;
        }
    }
    st.last_draw = Some(t);

    let dt = st.last_tick.map_or(0.033, |l| (t - l).min(0.1));
    st.last_tick = Some(t);
    st.phase += dt;
    if diff.abs() > 0.05 {
        st.shown += diff * (dt * 4.0).min(1.0);
    } else {
        st.shown = st.target;
    }

    if inten > 0.0 {
        if st.rng.next() < inten * 30.0 * dt {
            let p = Particle {
                x: st.rng.range(-1.0, 1.0),
                y: 0.0,
                vx: st.rng.range(-0.15, 0.15),
                vy: st.rng.range(0.35, 0.8),
                life: 1.0,
                size: st.rng.range(1.5, 4.0),
            };
            st.parts.push(p);
        }
        st.ring_clock += dt;
        if st.ring_clock > 1.2 - 0.6 * inten {
            st.ring_clock = 0.0;
            st.rings.push(0.0);
        }
    }
    for p in &mut st.parts {
        p.x += p.vx * dt;
        p.y += p.vy * dt;
        p.life -= dt * 0.9;
    }
    st.parts.retain(|p| p.life > 0.0);
    for r in &mut st.rings {
        *r += dt * 0.8;
    }
    st.rings.retain(|r| *r < 1.0);
    true
}

/// Si el tema de Omarchy cambió, recarga paleta y CSS sin reiniciar.
fn apply_theme(ui: &Rc<Ui>, force: bool) {
    let new = Palette::load();
    let font = ui.st.borrow().font.clone();
    if !force && ui.st.borrow().pal.raw == new.raw {
        return;
    }
    #[allow(deprecated)]
    ui.provider.load_from_data(&new.css(&font));
    adw::StyleManager::default().set_color_scheme(if new.dark {
        adw::ColorScheme::ForceDark
    } else {
        adw::ColorScheme::ForceLight
    });
    ui.theme_lbl.set_text(&new.name);
    ui.st.borrow_mut().pal = new;
    ui.hero.queue_draw();
    ui.spark.queue_draw();
    ensure_animating(ui);
    for m in &ui.minis {
        m.queue_draw();
    }
}

// ---------------------------------------------------------------- ventana

fn metric_card(title: &str) -> (gtk::Box, gtk::Label, gtk::Label, gtk::DrawingArea) {
    let c = gtk::Box::new(gtk::Orientation::Vertical, 1);
    c.add_css_class("card");
    c.set_hexpand(true);
    let l1 = gtk::Label::builder().label(title).xalign(0.0).build();
    l1.add_css_class("cardlabel");
    let v = gtk::Label::builder().label("--").xalign(0.0).build();
    v.add_css_class("cardvalue");
    let s = gtk::Label::builder().label(" ").xalign(0.0).ellipsize(pango::EllipsizeMode::End).build();
    s.add_css_class("cardsub");
    let sp = gtk::DrawingArea::builder().content_height(28).hexpand(true).margin_top(4).build();
    c.append(&l1);
    c.append(&v);
    c.append(&s);
    c.append(&sp);
    (c, v, s, sp)
}

fn build_ui(app: &adw::Application) {
    if let Some(w) = app.active_window() {
        w.present();
        return;
    }
    let provider = gtk::CssProvider::new();
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("sin display"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0x9E3779B97F4A7C15, |d| d.as_nanos() as u64 | 1);
    let st = Rc::new(RefCell::new(State {
        pal: Palette::load(),
        font: current_font(),
        target: 40.0,
        shown: 40.0,
        temp_hist: VecDeque::new(),
        cpu_hist: VecDeque::new(),
        ram_hist: VecDeque::new(),
        disk_hist: VecDeque::new(),
        groups: vec![],
        ram_total: 1.0,
        sort: 0,
        rows: HashMap::new(),
        row_keys: vec![],
        parts: vec![],
        rings: vec![],
        phase: 0.0,
        last_tick: None,
        last_draw: None,
        animating: false,
        ring_clock: 0.0,
        rng: Rng(seed),
        sensors: Sensors::new(),
        sys: SysSampler::default(),
        sampler: ProcSampler::new(),
        culprit: None,
    }));

    let win = adw::ApplicationWindow::builder()
        .application(app)
        .title("ThermalWatch")
        .default_width(480)
        .default_height(820)
        .build();
    win.add_css_class("tw");

    let toasts = adw::ToastOverlay::new();
    win.set_content(Some(&toasts));
    let outer = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).build();
    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);
    outer.set_child(Some(&vbox));
    toasts.set_child(Some(&outer));

    // Cabecera
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = gtk::Label::builder().label("󰔏 THERMALWATCH").xalign(0.0).hexpand(true).build();
    title.add_css_class("title");
    let theme_lbl = gtk::Label::builder().xalign(1.0).build();
    theme_lbl.add_css_class("themename");
    head.append(&title);
    head.append(&theme_lbl);
    vbox.append(&head);

    // Banner
    let revealer = gtk::Revealer::builder().transition_type(gtk::RevealerTransitionType::SlideDown).build();
    let bn = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    bn.add_css_class("banner");
    let banner_lbl = gtk::Label::builder().xalign(0.0).hexpand(true).wrap(true).build();
    let banner_btn = gtk::Button::with_label(tr("close_caps"));
    banner_btn.add_css_class("bannerbtn");
    bn.append(&banner_lbl);
    bn.append(&banner_btn);
    revealer.set_child(Some(&bn));
    vbox.append(&revealer);

    // Hero
    let hero = gtk::DrawingArea::builder().hexpand(true).content_height(230).build();
    let s = st.clone();
    hero.set_draw_func(move |_, cr, w, h| draw_hero(&s.borrow(), cr, w, h));
    vbox.append(&hero);

    let stat_lbl = gtk::Label::builder().label(" ").build();
    stat_lbl.add_css_class("statline");
    vbox.append(&stat_lbl);

    // Tarjetas: CPU, RAM, escritura de disco
    let cards = gtk::Box::builder().spacing(8).homogeneous(true).build();
    let (c1, v_cpu, s_cpu, sp1) = metric_card("CPU");
    let (c2, v_ram, s_ram, sp2) = metric_card("RAM");
    let (c3, v_disk, s_disk, sp3) = metric_card(tr("disk_w"));
    for c in [&c1, &c2, &c3] {
        cards.append(c);
    }
    vbox.append(&cards);
    let s = st.clone();
    sp1.set_draw_func(move |_, cr, w, h| {
        let s = s.borrow();
        draw_series(cr, w, h, &s.cpu_hist, MINI, (0.0, 100.0), s.pal.accent, None);
    });
    let s = st.clone();
    sp2.set_draw_func(move |_, cr, w, h| {
        let s = s.borrow();
        draw_series(cr, w, h, &s.ram_hist, MINI, (0.0, s.ram_total.max(1.0)), s.pal.magenta, None);
    });
    let s = st.clone();
    sp3.set_draw_func(move |_, cr, w, h| {
        let s = s.borrow();
        let hi = s.disk_hist.iter().copied().fold(5e6, f64::max);
        draw_series(cr, w, h, &s.disk_hist, MINI, (0.0, hi), s.pal.orange, None);
    });

    // Histórico de temperatura
    let spark = gtk::DrawingArea::builder().hexpand(true).content_height(56).build();
    let s = st.clone();
    spark.set_draw_func(move |_, cr, w, h| {
        let s = s.borrow();
        let last = s.temp_hist.back().copied().unwrap_or(40.0);
        draw_series(cr, w, h, &s.temp_hist, HIST, (T_MIN, T_MAX), s.pal.heat(last), Some((HOT, s.pal.red)));
    });
    vbox.append(&spark);

    // Procesos + pestañas de orden
    let row_h = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let h = gtk::Label::builder().label(tr("processes")).xalign(0.0).hexpand(true).build();
    h.add_css_class("section");
    row_h.append(&h);
    let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    tabs.add_css_class("tabs");
    let t_cpu = gtk::ToggleButton::builder().label("CPU").active(true).build();
    let t_ram = gtk::ToggleButton::builder().label("RAM").group(&t_cpu).build();
    let t_dsk = gtk::ToggleButton::builder().label(tr("disk_tab")).group(&t_cpu).build();
    tabs.append(&t_cpu);
    tabs.append(&t_ram);
    tabs.append(&t_dsk);
    row_h.append(&tabs);
    vbox.append(&row_h);

    let plist = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let sc = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(200)
        .build();
    sc.set_child(Some(&plist));
    vbox.append(&sc);

    let ui = Rc::new(Ui {
        st: st.clone(),
        win: win.clone(),
        provider,
        toasts,
        revealer,
        banner_lbl,
        theme_lbl,
        stat_lbl,
        hero: hero.clone(),
        spark,
        minis: vec![sp1, sp2, sp3],
        plist,
        v_cpu,
        s_cpu,
        v_ram,
        s_ram,
        v_disk,
        s_disk,
    });

    for (btn, idx) in [(&t_cpu, 0usize), (&t_ram, 1), (&t_dsk, 2)] {
        let u = ui.clone();
        btn.connect_toggled(move |b| {
            if b.is_active() {
                u.st.borrow_mut().sort = idx;
                render_procs(&u);
            }
        });
    }

    let u = ui.clone();
    banner_btn.connect_clicked(move |_| {
        let key = u.st.borrow().culprit.clone();
        if let Some(k) = key {
            confirm_kill(&u, &k);
        }
    });

    let u_init = ui.clone();
    ensure_animating(&u_init);

    apply_theme(&ui, true);
    update_sensors(&ui);
    sample_procs(&ui);
    let u = ui.clone();
    glib::timeout_add_seconds_local(1, move || {
        update_sensors(&u);
        glib::ControlFlow::Continue
    });
    let u = ui.clone();
    glib::timeout_add_seconds_local(3, move || {
        sample_procs(&u);
        glib::ControlFlow::Continue
    });
    let u = ui.clone();
    glib::timeout_add_seconds_local(6, move || {
        apply_theme(&u, false);
        glib::ControlFlow::Continue
    });

    win.present();
}

// ---------------------------------------------------------------- modo pánico

const PANIC_CSS: &str = r#"
window.panic { background: @tw_bg; color: @tw_fg; border: 2px solid @tw_red; }
window.panic label { font-family: "FONTFAMILY", monospace; }
.ptitle { color: @tw_red; font-weight: 800; letter-spacing: 3px; font-size: 15px; }
.psub { color: @tw_dfg; font-size: 11px; }
.pkey { background: @tw_red; color: @tw_bg; font-weight: 800; font-size: 16px; padding: 2px 12px; }
.pkey.off { background: @tw_muted; color: @tw_dfg; }
.pkind { color: @tw_dfg; font-weight: 700; letter-spacing: 1px; }
.pname { color: @tw_bfg; font-weight: 800; font-size: 15px; }
.pval { color: @tw_fg; }
.pdone { color: @tw_green; font-weight: 700; }
"#;

struct PanicState {
    targets: [Option<Group>; 3],
    ready: bool,
    sampler: ProcSampler,
}

fn pick(groups: &[Group], f: impl Fn(&Group) -> f64, min: f64) -> Option<Group> {
    groups
        .iter()
        .filter(|g| !g.protected && f(g) > min)
        .max_by(|a, b| f(a).partial_cmp(&f(b)).unwrap_or(std::cmp::Ordering::Equal))
        .cloned()
}

fn run_panic() -> glib::ExitCode {
    let app = adw::Application::builder().application_id("dev.ina.thermalwatch.panic").build();
    app.connect_activate(|app| {
        if let Some(w) = app.active_window() {
            w.present();
            return;
        }
        let pal = Palette::load();
        let prov = gtk::CssProvider::new();
        #[allow(deprecated)]
        prov.load_from_data(&(pal.css(&current_font()).replace("window.tw", "window.tw_unused") + &PANIC_CSS.replace("FONTFAMILY", &current_font())));
        gtk::style_context_add_provider_for_display(
            &gdk::Display::default().expect("sin display"),
            &prov,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        adw::StyleManager::default().set_color_scheme(if pal.dark {
            adw::ColorScheme::ForceDark
        } else {
            adw::ColorScheme::ForceLight
        });

        let win = adw::ApplicationWindow::builder()
            .application(app)
            .title("ThermalWatch · Kill")
            .default_width(560)
            .default_height(240)
            .resizable(false)
            .build();
        win.add_css_class("panic");
        let vb = gtk::Box::new(gtk::Orientation::Vertical, 12);
        vb.set_margin_top(20);
        vb.set_margin_bottom(20);
        vb.set_margin_start(20);
        vb.set_margin_end(20);
        let title = gtk::Label::builder().label(format!("󰆴 {}", tr("panic_title"))).xalign(0.0).build();
        title.add_css_class("ptitle");
        vb.append(&title);

        let kinds = [("R", "RAM"), ("C", "CPU"), ("D", tr("disk_tab"))];
        let mut names = vec![];
        let mut vals = vec![];
        let mut badges = vec![];
        for (k, kind) in kinds {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            let key = gtk::Label::builder().label(trf("press", &[k.to_string()])).build();
            key.add_css_class("pkey");
            key.add_css_class("off");
            let kd = gtk::Label::builder().label(kind).xalign(0.0).width_chars(6).build();
            kd.add_css_class("pkind");
            let name = gtk::Label::builder().label(tr("measuring")).xalign(0.0).hexpand(true)
                .ellipsize(pango::EllipsizeMode::End).build();
            name.add_css_class("pname");
            let val = gtk::Label::builder().xalign(1.0).build();
            val.add_css_class("pval");
            row.append(&key);
            row.append(&kd);
            row.append(&name);
            row.append(&val);
            vb.append(&row);
            badges.push(key);
            names.push(name);
            vals.push(val);
        }
        let status = gtk::Label::builder().label(tr("panic_hint")).xalign(0.0).wrap(true).build();
        status.add_css_class("psub");
        vb.append(&status);
        win.set_content(Some(&vb));

        let st = Rc::new(RefCell::new(PanicState { targets: [None, None, None], ready: false, sampler: ProcSampler::new() }));
        st.borrow_mut().sampler.sample();

        let (st2, names2, vals2, badges2) = (st.clone(), names.clone(), vals.clone(), badges.clone());
        glib::timeout_add_local_once(Duration::from_millis(900), move || {
            let mut s = st2.borrow_mut();
            let g = s.sampler.sample();
            let t = [
                pick(&g, |x| x.mem, 0.0),
                pick(&g, |x| x.cpu, 1.0),
                pick(&g, |x| x.wbps, 1024.0),
            ];
            let fmt = [
                t[0].as_ref().map(|x| format!("{} · {} proc", fmt_bytes(x.mem), x.pids.len())),
                t[1].as_ref().map(|x| format!("{:.0}% · {} proc", x.cpu, x.pids.len())),
                t[2].as_ref().map(|x| format!("{}/s · {} proc", fmt_bytes(x.wbps), x.pids.len())),
            ];
            for i in 0..3 {
                match &t[i] {
                    Some(x) => {
                        names2[i].set_text(&x.key);
                        vals2[i].set_text(fmt[i].as_deref().unwrap_or(""));
                        badges2[i].remove_css_class("off");
                    }
                    None => names2[i].set_text(tr("nothing")),
                }
            }
            s.targets = t;
            s.ready = true;
        });

        let ctrl = gtk::EventControllerKey::new();
        let (app2, st3, status2) = (app.clone(), st.clone(), status.clone());
        ctrl.connect_key_pressed(move |_, key, _, _| {
            let idx = match key {
                gdk::Key::r | gdk::Key::R => 0,
                gdk::Key::c | gdk::Key::C => 1,
                gdk::Key::d | gdk::Key::D => 2,
                gdk::Key::Escape => {
                    app2.quit();
                    return glib::Propagation::Stop;
                }
                _ => return glib::Propagation::Proceed,
            };
            let s = st3.borrow();
            if !s.ready {
                return glib::Propagation::Stop;
            }
            if let Some(g) = &s.targets[idx] {
                for &p in &g.pids {
                    unsafe { libc::kill(p, libc::SIGKILL) };
                }
                status2.set_text(&trf("killed", &[g.key.clone(), g.pids.len().to_string()]));
                status2.remove_css_class("psub");
                status2.add_css_class("pdone");
                let a = app2.clone();
                glib::timeout_add_local_once(Duration::from_millis(900), move || a.quit());
            }
            glib::Propagation::Stop
        });
        win.add_controller(ctrl);
        // Clic fuera (pérdida de foco) cierra el popup, una vez que ya tuvo el foco.
        let (app3, had_focus) = (app.clone(), Rc::new(std::cell::Cell::new(false)));
        win.connect_is_active_notify(move |w| {
            if w.is_active() {
                had_focus.set(true);
            } else if had_focus.get() {
                app3.quit();
            }
        });
        win.present();
    });
    app.run_with_args::<String>(&[])
}

fn main() -> glib::ExitCode {
    if std::env::args().any(|a| a == "--panic") {
        return run_panic();
    }
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run_with_args::<String>(&[])
}
