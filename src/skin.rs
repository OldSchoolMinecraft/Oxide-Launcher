//! The Skin Editor tab: paint a skin, pick a cloak and preview both on the
//! animated Oxide player model.
use crate::{
    chrome::{self, BOLD, color},
    profile::group,
    settings::SkinSource,
    skin_model::{self, Motion, Options, SIZE, Scene, Texture, WALK_AMOUNT, Walk},
};
use iced::{
    Color, Element, Fill, Point, Rectangle, Renderer, Size, Task, Theme, mouse,
    widget::{
        button, canvas, checkbox, column, container, image, pick_list, row, scrollable, slider,
        stack, text, text_input, tooltip,
    },
};
use std::{
    cell::RefCell,
    fmt, fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const MAX_PNG: usize = 1024 * 1024;
const MAX_UNDO: usize = 64;
const MAX_HISTORY: usize = 18;
const COSMETICS: &str = "https://micro.os-mc.net/cosmetics";
/// Swatches shown when the texture has too few colors of its own.
const PALETTE: [u32; 16] = [
    0x000000, 0x3f3f3f, 0x7f7f7f, 0xffffff, 0x8b4513, 0xc6966c, 0xf0c8a0, 0xb7410e, 0xd32f2f,
    0xf57c00, 0xfbc02d, 0x388e3c, 0x00897b, 0x1976d2, 0x3949ab, 0x8e24aa,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tool {
    Pencil,
    Eraser,
    Fill,
    Picker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    Skin,
    Cape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CapeChoice {
    None,
    /// A PNG in the launcher's cloak library, by file stem.
    Library(String),
}
impl fmt::Display for CapeChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::None => "No cape",
            Self::Library(name) => name,
        })
    }
}

impl Walk {
    const ALL: [Self; 2] = [Self::Normal, Self::Classic];
}
impl fmt::Display for Walk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Normal => "Normal walk",
            Self::Classic => "Classic walk (Oxide perk)",
        })
    }
}

/// A named PNG chosen by the user; `None` when the dialog was cancelled.
type Picked = Result<Option<(String, Vec<u8>)>, String>;

#[derive(Debug, Clone, Default)]
pub(crate) struct Cosmetics {
    skin: Option<Vec<u8>>,
    cape: Option<Vec<u8>>,
    /// The arm model, when the service states it (Mojang does).
    slim: Option<bool>,
}

/// The answer to a cosmetics lookup, with the username it was made for.
#[derive(Debug, Clone)]
pub(crate) struct Account {
    name: String,
    source: SkinSource,
    cosmetics: Result<Cosmetics, String>,
}

#[derive(Debug, Clone)]
pub(crate) enum Msg {
    Tool(Tool),
    Target(Target),
    Color([u8; 3]),
    Channel(usize, u8),
    Hex(String),
    Paint {
        x: usize,
        y: usize,
        start: bool,
    },
    Pick {
        x: usize,
        y: usize,
    },
    StrokeEnd,
    Undo,
    Redo,
    Slim(bool),
    Overlay(bool),
    Guides(bool),
    Walk(Walk),
    TogglePlay,
    Frame(Instant),
    OpenSkin,
    SkinOpened(Picked),
    Reset,
    Export,
    Exported(Result<Option<PathBuf>, String>),
    LoadAccount,
    Lookup(String),
    AccountLoaded(Account),
    Cape(CapeChoice),
    UploadCape,
    CapeUploaded(Picked),
    Save,
    Source(SkinSource),
    /// Wear the skin at this position in the history.
    History(usize),
}

/// A skin kept in the history.
struct Saved {
    name: String,
    sequence: u32,
    texture: Texture,
    slim: bool,
    icon: image::Handle,
    file: Option<PathBuf>,
}

struct Snapshot {
    target: Target,
    texture: Texture,
}

pub(crate) struct Editor {
    skin: Texture,
    cape: Option<Texture>,
    cape_choice: CapeChoice,
    library: Vec<String>,
    /// A thumbnail for each library cape, in the same order.
    cape_icons: Vec<image::Handle>,
    /// Skins that were loaded, saved or replaced while edited, newest first.
    history: Vec<Saved>,
    source: SkinSource,
    slim: bool,
    overlay: bool,
    guides: bool,
    target: Target,
    tool: Tool,
    color: [u8; 3],
    hex: String,
    walk: Walk,
    playing: bool,
    phase: f32,
    amount: f32,
    age: f32,
    last_frame: Option<Instant>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    stroke: Option<(usize, usize)>,
    status: String,
    busy: bool,
    /// The valid username in the launcher's username box.
    wanted: Option<String>,
    /// The username whose cosmetics are being fetched; other answers are stale.
    loading: Option<String>,
    /// Whether the skin has local changes, which are kept as the draft.
    edited: bool,
    /// Bumped whenever the preview may look different.
    revision: u64,
    /// The texture being painted, composited over a transparency checkerboard.
    sheet: image::Handle,
    /// Swatches drawn from the texture being painted.
    palette: Vec<[u8; 3]>,
    /// `skin-editor` in the launcher data directory: the draft skin and cloak library.
    directory: Option<PathBuf>,
}

impl Editor {
    pub fn new(data: Option<&Path>) -> Self {
        let directory = data.map(|data| data.join("skin-editor"));
        let draft = directory
            .as_ref()
            .and_then(|directory| fs::read(directory.join("skin.png")).ok())
            .and_then(|bytes| decode(&bytes, false).ok());
        let (skin, slim) = draft.unwrap_or_else(|| (default_skin(), false));
        let mut editor = Self {
            sheet: sheet(&skin),
            palette: palette(&skin),
            skin,
            cape: None,
            cape_choice: CapeChoice::None,
            library: Vec::new(),
            cape_icons: Vec::new(),
            history: Vec::new(),
            source: SkinSource::default(),
            slim,
            overlay: true,
            guides: true,
            target: Target::Skin,
            tool: Tool::Pencil,
            color: [0xb7, 0x41, 0x0e],
            hex: String::new(),
            walk: Walk::Normal,
            playing: false,
            phase: 0.,
            amount: 0.,
            age: 0.,
            last_frame: None,
            undo: Vec::new(),
            redo: Vec::new(),
            stroke: None,
            status: "Drag the model to rotate it. Right-click the texture to pick a color.".into(),
            busy: false,
            wanted: None,
            loading: None,
            edited: false,
            revision: 0,
            directory,
        };
        editor.set_color(editor.color);
        editor.scan_library();
        editor.load_history();
        editor.edited = editor
            .directory
            .as_ref()
            .is_some_and(|directory| directory.join("skin.png").exists());
        editor
    }

    /// Shows the launch username's skin, unless an edited draft is waiting.
    pub fn start(&mut self, username: Option<&str>) -> Task<Msg> {
        self.wanted = username.map(str::to_owned);
        match &self.wanted {
            Some(name) if !self.edited => self.fetch(name.clone()),
            Some(name) => {
                self.status = format!(
                    "Showing your edited draft. Load account skin & cape replaces it with \
                     {name}'s current skin."
                );
                Task::none()
            }
            None => Task::none(),
        }
    }

    /// Follows the username box, looking the name up once typing pauses.
    pub fn username_changed(&mut self, username: Option<&str>) -> Task<Msg> {
        if self.wanted.as_deref() == username {
            return Task::none();
        }
        self.wanted = username.map(str::to_owned);
        match self.wanted.clone() {
            Some(name) => {
                // The timer may only be created once the task runs on the runtime.
                let pause = async { tokio::time::sleep(Duration::from_millis(600)).await };
                Task::perform(pause, move |()| Msg::Lookup(name.clone()))
            }
            None => Task::none(),
        }
    }

    pub fn with_source(mut self, source: SkinSource) -> Self {
        self.source = source;
        self
    }

    fn fetch(&mut self, name: String) -> Task<Msg> {
        self.status = format!("Loading {name}'s skin and cape from {}…", self.source);
        self.loading = Some(name.clone());
        Task::perform(account(name, self.source), Msg::AccountLoaded)
    }

    fn history_directory(&self) -> Option<PathBuf> {
        self.directory
            .as_ref()
            .map(|directory| directory.join("history"))
    }

    /// History files are `<sequence>-<name>.png`; the highest sequence is newest.
    fn load_history(&mut self) {
        let mut files: Vec<PathBuf> = self
            .history_directory()
            .and_then(|directory| fs::read_dir(directory).ok())
            .into_iter()
            .flatten()
            .filter_map(|entry| Some(entry.ok()?.path()))
            .filter(|path| path.extension().is_some_and(|kind| kind == "png"))
            .collect();
        files.sort();
        files.reverse();
        self.history = files
            .into_iter()
            .filter_map(|file| {
                let (sequence, name) = file.file_stem()?.to_str()?.split_once('-')?;
                let (texture, slim) = decode(&fs::read(&file).ok()?, false).ok()?;
                Some(Saved {
                    name: name.to_owned(),
                    sequence: sequence.parse().ok()?,
                    icon: face(&texture),
                    texture,
                    slim,
                    file: Some(file),
                })
            })
            .collect();
        self.trim_history();
    }

    fn trim_history(&mut self) {
        for old in self.history.drain(MAX_HISTORY.min(self.history.len())..) {
            if let Some(file) = old.file {
                let _ = fs::remove_file(file);
            }
        }
    }

    /// Puts the current skin at the front of the history under `name`.
    fn record(&mut self, name: &str) {
        let name = library_name(name);
        let sequence = self.history.first().map_or(1, |newest| newest.sequence + 1);
        // The same skin is listed once, under its latest name.
        self.history.retain(|saved| {
            let same = saved.texture == self.skin;
            if same && let Some(file) = &saved.file {
                let _ = fs::remove_file(file);
            }
            !same
        });
        let file = self.history_directory().and_then(|directory| {
            let file = directory.join(format!("{sequence:08}-{name}.png"));
            fs::create_dir_all(&directory).ok()?;
            fs::write(&file, encode(&self.skin).ok()?).ok()?;
            Some(file)
        });
        self.history.insert(
            0,
            Saved {
                name,
                sequence,
                icon: face(&self.skin),
                texture: self.skin.clone(),
                slim: self.slim,
                file,
            },
        );
        self.trim_history();
    }

    /// Keeps unsaved work reachable before another skin takes its place.
    fn record_edits(&mut self) {
        // A skin that is already listed keeps the name it was listed under.
        if self.edited && !self.history.iter().any(|saved| saved.texture == self.skin) {
            self.record("Edited skin");
        }
    }

    /// Replaces the skin and cape with what the account wears in game.
    fn wear(&mut self, account: Account) {
        let name = account.name;
        let cosmetics = match account.cosmetics {
            Ok(cosmetics) => cosmetics,
            Err(error) => {
                self.status = format!("Could not load {name}'s skin: {error}");
                return;
            }
        };
        let source = account.source;
        let mut notes = Vec::new();
        match &cosmetics.skin {
            Some(skin) => {
                if self.load_skin(skin, &format!("{name} ({source})"), cosmetics.slim) {
                    self.discard_draft();
                }
                notes.push(self.status.clone());
            }
            // Nothing of our own to lose: show what the game would.
            None if !self.edited => {
                self.target = Target::Skin;
                self.remember();
                self.skin = default_skin();
                self.slim = false;
                notes.push(format!("{name} has no {source} skin."));
            }
            None => notes.push(format!(
                "{name} has no {source} skin; your edited skin was kept."
            )),
        }
        match &cosmetics.cape {
            Some(cape) => {
                if let Err(error) = self.add_cape(&format!("{name} ({source})"), cape) {
                    notes.push(format!("Their cape could not be loaded: {error}"));
                }
            }
            None => self.set_cape(CapeChoice::None, None),
        }
        self.status = notes.join(" ");
    }

    fn discard_draft(&mut self) {
        self.edited = false;
        if let Some(directory) = &self.directory {
            let _ = fs::remove_file(directory.join("skin.png"));
        }
    }

    /// Whether the preview needs animation frames.
    pub fn animating(&self) -> bool {
        self.playing || self.amount > 0.001
    }

    fn active(&self) -> &Texture {
        match (self.target, &self.cape) {
            (Target::Cape, Some(cape)) => cape,
            _ => &self.skin,
        }
    }

    fn active_mut(&mut self) -> &mut Texture {
        match (self.target, &mut self.cape) {
            (Target::Cape, Some(cape)) => cape,
            _ => &mut self.skin,
        }
    }

    fn set_color(&mut self, color: [u8; 3]) {
        self.color = color;
        self.hex = format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2]);
    }

    fn capes_directory(&self) -> Option<PathBuf> {
        self.directory
            .as_ref()
            .map(|directory| directory.join("capes"))
    }

    fn scan_library(&mut self) {
        self.library = self
            .capes_directory()
            .and_then(|directory| fs::read_dir(directory).ok())
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                (path.extension()?.to_str()?.eq_ignore_ascii_case("png"))
                    .then(|| path.file_stem()?.to_str().map(str::to_owned))?
            })
            .collect();
        self.library.sort_by_key(|name| name.to_lowercase());
        self.cape_icons = self
            .library
            .iter()
            .map(|name| {
                let cape = self
                    .capes_directory()
                    .and_then(|directory| fs::read(directory.join(format!("{name}.png"))).ok())
                    .and_then(|bytes| decode(&bytes, true).ok());
                cape_front(&cape.map_or_else(|| Texture::new(32), |(cape, _)| cape))
            })
            .collect();
    }

    fn remember(&mut self) {
        self.undo.push(Snapshot {
            target: self.target,
            texture: self.active().clone(),
        });
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Swaps the newest snapshot of `from` with the live texture, keeping it for `to`.
    fn restore(&mut self, redo: bool) {
        let (from, to) = if redo {
            (&mut self.redo, &mut self.undo)
        } else {
            (&mut self.undo, &mut self.redo)
        };
        let Some(snapshot) = from.pop() else { return };
        let live = match (snapshot.target, &mut self.cape) {
            (Target::Cape, Some(cape)) => cape,
            (Target::Cape, None) => return,
            (Target::Skin, _) => &mut self.skin,
        };
        to.push(Snapshot {
            target: snapshot.target,
            texture: std::mem::replace(live, snapshot.texture),
        });
        self.target = snapshot.target;
        self.changed();
    }

    /// The skin draft survives restarts until account saving exists.
    fn changed(&mut self) {
        self.edited = true;
        if let Some(directory) = &self.directory
            && let Err(error) = fs::create_dir_all(directory)
                .map_err(|error| error.to_string())
                .and_then(|()| encode(&self.skin))
                .and_then(|png| {
                    fs::write(directory.join("skin.png"), png).map_err(|error| error.to_string())
                })
        {
            self.status = format!("Could not keep the skin draft: {error}");
        }
    }

    fn set_cape(&mut self, choice: CapeChoice, cape: Option<Texture>) {
        self.cape = cape;
        self.cape_choice = choice;
        self.undo.retain(|snapshot| snapshot.target == Target::Skin);
        self.redo.retain(|snapshot| snapshot.target == Target::Skin);
        if self.cape.is_none() {
            self.target = Target::Skin;
        }
    }

    /// Wears a skin PNG and lists it in the history as `source`. `model` is the
    /// arm model when the provider states it. Returns whether the PNG was usable.
    fn load_skin(&mut self, bytes: &[u8], source: &str, model: Option<bool>) -> bool {
        match decode(bytes, false) {
            Ok((skin, slim)) => {
                let slim = model.map_or(slim, |slim| slim && skin.height == SIZE);
                self.record_edits();
                self.target = Target::Skin;
                self.remember();
                self.status = format!(
                    "Loaded {source} ({SIZE}×{}{}).",
                    skin.height,
                    if slim { ", slim arms" } else { "" }
                );
                self.skin = skin;
                self.slim = slim;
                self.record(source);
                true
            }
            Err(error) => {
                self.status = format!("Could not load {source}: {error}");
                false
            }
        }
    }

    /// Adds a cloak PNG to the library and wears it.
    fn add_cape(&mut self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let (cape, _) = decode(bytes, true)?;
        let name = library_name(name);
        let stored = self.capes_directory().is_some_and(|directory| {
            fs::create_dir_all(&directory).is_ok()
                && fs::write(directory.join(format!("{name}.png")), bytes).is_ok()
        });
        self.scan_library();
        self.set_cape(CapeChoice::Library(name.clone()), Some(cape));
        self.status = if stored {
            format!("Added cape “{name}” to your library.")
        } else {
            format!("Wearing “{name}”, but it could not be added to the cape library.")
        };
        Ok(())
    }

    fn paint(&mut self, x: usize, y: usize, start: bool) {
        let height = self.active().height;
        if x >= SIZE || y >= height {
            return;
        }
        let ink = match self.tool {
            Tool::Pencil => [self.color[0], self.color[1], self.color[2], 255],
            Tool::Eraser => [0; 4],
            Tool::Fill => {
                if start {
                    self.remember();
                    let ink = [self.color[0], self.color[1], self.color[2], 255];
                    flood(self.active_mut(), x, y, ink);
                }
                return;
            }
            Tool::Picker => {
                self.pick(x, y);
                return;
            }
        };
        let from = match self.stroke {
            Some(from) if !start => from,
            _ => {
                self.remember();
                (x, y)
            }
        };
        self.stroke = Some((x, y));
        let texture = self.active_mut();
        // Fast drags skip texels; join the samples.
        let (dx, dy) = (x as i32 - from.0 as i32, y as i32 - from.1 as i32);
        let steps = dx.abs().max(dy.abs()).max(1);
        for step in 0..=steps {
            let x = from.0 as i32 + (dx * step + steps / 2 * dx.signum()) / steps;
            let y = from.1 as i32 + (dy * step + steps / 2 * dy.signum()) / steps;
            texture.set(x as usize, y as usize, ink);
        }
    }

    fn pick(&mut self, x: usize, y: usize) {
        let texture = self.active();
        if x < SIZE && y < texture.height {
            let [r, g, b, a] = texture.get(x, y);
            if a != 0 {
                self.set_color([r, g, b]);
            }
        }
    }

    pub fn update(&mut self, message: Msg, username: Option<String>) -> Task<Msg> {
        // Most messages leave the textures alone: keep the rendered preview and
        // the paint sheet for those.
        // The swatches wait for a stroke to end, so they hold still under the brush.
        let swatches = !matches!(message, Msg::Paint { .. });
        let (preview, repaint) = match &message {
            Msg::Frame(_) | Msg::Slim(_) | Msg::Overlay(_) | Msg::Walk(_) => (true, false),
            Msg::Target(_) | Msg::StrokeEnd => (false, true),
            Msg::Paint { .. }
            | Msg::Undo
            | Msg::Redo
            | Msg::SkinOpened(_)
            | Msg::Reset
            | Msg::History(_)
            | Msg::AccountLoaded(_)
            | Msg::Cape(_)
            | Msg::CapeUploaded(_) => (true, true),
            _ => (false, false),
        };
        let task = self.handle(message, username);
        if preview {
            self.revision += 1;
        }
        if repaint {
            self.sheet = sheet(self.active());
            if swatches {
                self.palette = palette(self.active());
            }
        }
        task
    }

    fn handle(&mut self, message: Msg, username: Option<String>) -> Task<Msg> {
        match message {
            Msg::Tool(tool) => self.tool = tool,
            Msg::Target(target) => {
                if target == Target::Skin || self.cape.is_some() {
                    self.target = target;
                }
            }
            Msg::Color(color) => self.set_color(color),
            Msg::Channel(channel, value) => {
                let mut color = self.color;
                color[channel] = value;
                self.set_color(color);
            }
            Msg::Hex(hex) => {
                let digits = hex.trim().trim_start_matches('#');
                if digits.len() == 6
                    && let Ok(value) = u32::from_str_radix(digits, 16)
                {
                    self.color = [(value >> 16) as u8, (value >> 8) as u8, value as u8];
                }
                self.hex = hex;
            }
            Msg::Paint { x, y, start } => self.paint(x, y, start),
            Msg::Pick { x, y } => self.pick(x, y),
            Msg::StrokeEnd => {
                self.stroke = None;
                self.changed();
            }
            Msg::Undo => self.restore(false),
            Msg::Redo => self.restore(true),
            Msg::Slim(slim) => self.slim = slim,
            Msg::Overlay(overlay) => self.overlay = overlay,
            Msg::Guides(guides) => self.guides = guides,
            Msg::Walk(walk) => self.walk = walk,
            Msg::TogglePlay => {
                self.playing = !self.playing;
                self.last_frame = None;
            }
            Msg::Frame(now) => {
                let elapsed = self
                    .last_frame
                    .map_or(Duration::ZERO, |last| now.saturating_duration_since(last))
                    .min(Duration::from_millis(100));
                self.last_frame = Some(now);
                let ticks = elapsed.as_secs_f32() * 20.;
                let target = if self.playing { WALK_AMOUNT } else { 0. };
                // EntityLiving eases the limb amount toward the speed by 0.4 a tick.
                self.amount += (target - self.amount) * (1. - 0.6_f32.powf(ticks));
                self.phase += self.amount * ticks;
                self.age += ticks;
            }
            Msg::OpenSkin if !self.busy => {
                self.busy = true;
                return Task::perform(pick_png("Open skin"), Msg::SkinOpened);
            }
            Msg::SkinOpened(result) => {
                self.busy = false;
                match result {
                    Ok(Some((name, bytes))) => {
                        if self.load_skin(&bytes, &name, None) {
                            self.changed();
                        }
                    }
                    Ok(None) => {}
                    Err(error) => self.status = error,
                }
            }
            Msg::Source(source) => {
                self.source = source;
                if let Some(name) = self.wanted.clone() {
                    return self.fetch(name);
                }
            }
            Msg::History(index) => {
                if let Some(saved) = self.history.get(index) {
                    let (texture, slim, name) =
                        (saved.texture.clone(), saved.slim, saved.name.clone());
                    self.record_edits();
                    self.target = Target::Skin;
                    self.remember();
                    self.skin = texture;
                    self.slim = slim;
                    self.status = format!("Wearing “{name}” from your history.");
                    self.changed();
                }
            }
            Msg::Reset => {
                self.record_edits();
                self.target = Target::Skin;
                self.remember();
                self.skin = default_skin();
                self.slim = false;
                self.status = "Started a new 64×32 skin.".into();
                self.changed();
            }
            Msg::Export if !self.busy => match encode(self.active()) {
                Ok(png) => {
                    self.busy = true;
                    let name = match self.target {
                        Target::Cape if self.cape.is_some() => "cape.png",
                        _ => "skin.png",
                    };
                    return Task::perform(export(name, png), Msg::Exported);
                }
                Err(error) => self.status = error,
            },
            Msg::Exported(result) => {
                self.busy = false;
                match result {
                    Ok(Some(path)) => self.status = format!("Exported {}.", path.display()),
                    Ok(None) => {}
                    Err(error) => self.status = format!("Export failed: {error}"),
                }
            }
            Msg::LoadAccount => match username {
                Some(name) => return self.fetch(name),
                None => self.status = "Enter a valid username first.".into(),
            },
            Msg::Lookup(name) => {
                // Only the name still in the box once typing pauses is looked up.
                if self.wanted.as_deref() == Some(&name) {
                    return self.fetch(name);
                }
            }
            Msg::AccountLoaded(account) => {
                if self.loading.as_deref() == Some(&account.name) && account.source == self.source {
                    self.loading = None;
                    self.wear(account);
                }
            }
            Msg::Cape(CapeChoice::None) => self.set_cape(CapeChoice::None, None),
            Msg::Cape(CapeChoice::Library(name)) => {
                let cape = self
                    .capes_directory()
                    .ok_or_else(|| "The cape library is unavailable".to_owned())
                    .and_then(|directory| {
                        fs::read(directory.join(format!("{name}.png")))
                            .map_err(|error| error.to_string())
                    })
                    .and_then(|bytes| decode(&bytes, true));
                match cape {
                    Ok((cape, _)) => self.set_cape(CapeChoice::Library(name), Some(cape)),
                    Err(error) => self.status = format!("Could not load cape “{name}”: {error}"),
                }
            }
            Msg::UploadCape if !self.busy => {
                self.busy = true;
                return Task::perform(pick_png("Upload cape"), Msg::CapeUploaded);
            }
            Msg::CapeUploaded(result) => {
                self.busy = false;
                match result {
                    Ok(Some((name, bytes))) => {
                        if let Err(error) = self.add_cape(&name, &bytes) {
                            self.status = format!("Could not load {name}: {error}");
                        }
                    }
                    Ok(None) => {}
                    Err(error) => self.status = error,
                }
            }
            Msg::Save => {
                // Placeholder: once the website API accepts cosmetics, send the
                // encoded skin (`encode(&self.skin)`), the selected cape and the arm
                // model for `username` here.
                self.record("Saved skin");
                self.status =
                    "Saving to your Oxide account isn't available yet. Your skin is kept as a \
                     local draft and in the history; use Export to keep a copy."
                        .into();
            }
            Msg::OpenSkin | Msg::Export | Msg::UploadCape => {}
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Msg> {
        column![
            row![self.preview(), self.painter(), self.sidebar()]
                .spacing(10)
                .height(Fill),
            text(&self.status).size(11),
        ]
        .spacing(5)
        .padding(iced::padding::all(8).top(4).bottom(5))
        .into()
    }

    fn preview(&self) -> Element<'_, Msg> {
        let preview = column![
            container(canvas(Viewer(self)).width(Fill).height(Fill))
                .padding(1)
                .style(chrome::frame)
                .height(Fill),
            row![
                push(
                    if self.playing { "Stop" } else { "Walk" },
                    Some(Msg::TogglePlay)
                )
                .width(56),
                pick_list(Walk::ALL, Some(self.walk), Msg::Walk)
                    .text_size(12)
                    .padding([4, 6])
                    .width(Fill)
                    .style(chrome::select),
            ]
            .spacing(6)
            .align_y(iced::Center),
            row![
                toggle("Slim arms", self.slim, Msg::Slim),
                toggle("Outer layer", self.overlay, Msg::Overlay),
            ]
            .spacing(12),
        ]
        .spacing(7);
        container(group("Preview", preview.into()))
            .width(232)
            .into()
    }

    fn painter(&self) -> Element<'_, Msg> {
        let texture = self.active();
        let cape = self.cape.is_some();
        let painting_cape = cape && self.target == Target::Cape;
        let tools = Tool::ALL
            .into_iter()
            .fold(row![].spacing(2), |tools, tool| {
                tools.push(tipped(
                    choice(tool.label(), self.tool == tool, Some(Msg::Tool(tool))),
                    tool.hint(),
                ))
            });
        let toolbar = row![
            row![
                tipped(
                    choice("Skin", !painting_cape, Some(Msg::Target(Target::Skin))),
                    "Paint the skin",
                ),
                tipped(
                    choice(
                        "Cape",
                        painting_cape,
                        cape.then_some(Msg::Target(Target::Cape))
                    ),
                    if cape {
                        "Paint the selected cape"
                    } else {
                        "Select or upload a cape to paint it"
                    },
                ),
            ]
            .spacing(2),
            tools,
            row![
                tipped(
                    choice("Undo", false, (!self.undo.is_empty()).then_some(Msg::Undo)),
                    "Undo (Ctrl+Z)",
                ),
                tipped(
                    choice("Redo", false, (!self.redo.is_empty()).then_some(Msg::Redo)),
                    "Redo (Ctrl+Y)",
                ),
            ]
            .spacing(2),
            tipped(
                toggle("Guides", self.guides, Msg::Guides),
                "Outline each body part's faces",
            ),
        ]
        .spacing(10)
        .align_y(iced::Center)
        .wrap()
        .vertical_spacing(4);
        let guides = match (self.guides, painting_cape) {
            (false, _) => Vec::new(),
            (true, true) => skin_model::cape_guides(),
            (true, false) => skin_model::skin_guides(texture.height == SIZE),
        };
        let layout = if painting_cape {
            "Cape · 22×17 area".to_owned()
        } else {
            format!("Skin · {SIZE}×{}", texture.height)
        };
        let painter = column![
            toolbar,
            container(stack![
                canvas(Sheet {
                    image: &self.sheet,
                    height: texture.height
                })
                .width(Fill)
                .height(Fill),
                canvas(Grid {
                    height: texture.height,
                    guides
                })
                .width(Fill)
                .height(Fill),
            ])
            .padding(4)
            .width(Fill)
            .height(Fill)
            .style(chrome::frame),
            row![
                text(self.tool.hint())
                    .size(11)
                    .style(chrome::muted_text)
                    .width(Fill),
                text(layout).size(11).style(chrome::muted_text),
            ]
            .spacing(8),
        ]
        .spacing(6);
        container(group("Texture", painter.into()))
            .width(Fill)
            .into()
    }

    fn sidebar(&self) -> Element<'_, Msg> {
        let [r, g, b] = self.color;
        let swatch = move |_: &Theme| {
            container::Style::default()
                .background(Color::from_rgb8(r, g, b))
                .border(iced::Border {
                    color: color(0x8f8f8f),
                    width: 1.,
                    radius: 2.into(),
                })
        };
        let channels = ["R", "G", "B"].into_iter().enumerate().fold(
            column![].spacing(2),
            |channels, (index, label)| {
                channels.push(
                    row![
                        text(label).size(11).width(10),
                        slider(0..=255, self.color[index], move |value| Msg::Channel(
                            index, value
                        )),
                        text(self.color[index]).size(11).width(22),
                    ]
                    .spacing(6)
                    .align_y(iced::Center),
                )
            },
        );
        let palette = self
            .palette
            .chunks(8)
            .fold(column![].spacing(2), |palette, colors| {
                palette.push(colors.iter().fold(row![].spacing(2), |line, &rgb| {
                    line.push(
                        button("")
                            .width(18)
                            .height(18)
                            .padding(0)
                            .style(move |_, _| button::Style {
                                background: Some(Color::from_rgb8(rgb[0], rgb[1], rgb[2]).into()),
                                border: iced::Border {
                                    color: color(0x6f6f6f),
                                    width: 1.,
                                    radius: 0.into(),
                                },
                                ..Default::default()
                            })
                            .on_press(Msg::Color(rgb)),
                    )
                }))
            });
        let paint = column![
            row![
                container("").width(26).height(24).style(swatch),
                text_input("#rrggbb", &self.hex)
                    .on_input(Msg::Hex)
                    .padding([4, 6])
                    .size(12)
                    .style(chrome::input),
            ]
            .spacing(6)
            .align_y(iced::Center),
            channels,
            palette,
        ]
        .spacing(6);
        let idle = !self.busy;
        let skin = column![
            row![
                text("Source").size(12),
                pick_list(SkinSource::ALL, Some(self.source), Msg::Source)
                    .text_size(12)
                    .padding([4, 6])
                    .width(Fill)
                    .style(chrome::select),
            ]
            .spacing(8)
            .align_y(iced::Center),
            tipped(
                push("Reload from account", Some(Msg::LoadAccount)).width(Fill),
                "Replace the skin and cape with the ones this username has at the source",
            ),
            row![
                tipped(
                    push("Open…", idle.then_some(Msg::OpenSkin)).width(Fill),
                    "Load a skin PNG from your computer",
                ),
                tipped(
                    push("Export…", idle.then_some(Msg::Export)).width(Fill),
                    "Save what you are painting as a PNG",
                ),
            ]
            .spacing(6),
            tipped(
                push("Reset to default", Some(Msg::Reset)).width(Fill),
                "Start again from the plain default skin",
            ),
        ]
        .spacing(6);
        let mut capes = vec![CapeChoice::None];
        capes.extend(self.library.iter().cloned().map(CapeChoice::Library));
        let cape = column![
            pick_list(capes, Some(self.cape_choice.clone()), Msg::Cape)
                .text_size(12)
                .padding([4, 6])
                .width(Fill)
                .style(chrome::select),
            tipped(
                push("Upload cape…", idle.then_some(Msg::UploadCape)).width(Fill),
                "Add a cape PNG (64×32 or 22×17) to your library and wear it",
            ),
        ]
        .spacing(6);
        let thumbnail = |icon: &image::Handle, width: f32, message: Msg| {
            button(
                image(icon.clone())
                    .width(width)
                    .height(24)
                    .filter_method(image::FilterMethod::Nearest),
            )
            .padding(1)
            .style(|theme, status| button::Style {
                border: iced::Border {
                    color: match status {
                        button::Status::Hovered | button::Status::Pressed => color(0x4a9eea),
                        _ => chrome::muted(theme),
                    },
                    width: 1.,
                    radius: 0.into(),
                },
                ..Default::default()
            })
            .on_press(message)
        };
        let skins =
            self.history
                .iter()
                .enumerate()
                .fold(row![].spacing(3), |skins, (index, saved)| {
                    skins.push(tipped(
                        thumbnail(&saved.icon, 24., Msg::History(index)),
                        &saved.name,
                    ))
                });
        let worn = self.library.iter().zip(&self.cape_icons).fold(
            row![].spacing(3),
            |capes, (name, icon)| {
                capes.push(tipped(
                    thumbnail(icon, 15., Msg::Cape(CapeChoice::Library(name.clone()))),
                    name,
                ))
            },
        );
        let caption = |label| text(label).size(11).style(chrome::muted_text);
        let history = column![
            caption(if self.history.is_empty() {
                "Skins you load or save appear here."
            } else {
                "Skins"
            }),
            skins.wrap().vertical_spacing(3),
            caption(if self.library.is_empty() {
                "Capes you upload appear here."
            } else {
                "Capes"
            }),
            worn.wrap().vertical_spacing(3),
        ]
        .spacing(4);
        let groups = column![
            group("Color", paint.into()),
            group("Skin", skin.into()),
            group("Cape", cape.into()),
            group("History", history.into()),
        ]
        .spacing(7);
        // Saving stays in reach however short the window is.
        column![
            scrollable(container(groups).padding(iced::padding::right(12)))
                .spacing(3)
                .height(Fill),
            container(
                button(container(text("Save to account").size(13).font(BOLD)).center_x(Fill))
                    .padding([7, 8])
                    .width(Fill)
                    .style(chrome::control)
                    .on_press(Msg::Save),
            )
            .padding(iced::padding::right(15)),
        ]
        .spacing(6)
        .width(222)
        .into()
    }
}

impl Tool {
    const ALL: [Self; 4] = [Self::Pencil, Self::Eraser, Self::Fill, Self::Picker];

    fn label(self) -> &'static str {
        match self {
            Self::Pencil => "Pencil",
            Self::Eraser => "Eraser",
            Self::Fill => "Fill",
            Self::Picker => "Picker",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Self::Pencil => "Click or drag to paint. Right-click picks a color.",
            Self::Eraser => "Click or drag to make pixels transparent.",
            Self::Fill => "Click to fill a connected area of one color.",
            Self::Picker => "Click a pixel to paint with its color.",
        }
    }
}

/// Explains a control when the pointer rests on it.
fn tipped<'a>(content: impl Into<Element<'a, Msg>>, tip: &'a str) -> Element<'a, Msg> {
    tooltip(
        content,
        container(text(tip).size(11))
            .padding([3, 6])
            .style(chrome::tip),
        tooltip::Position::Bottom,
    )
    .gap(4)
    .into()
}

fn push(label: &str, message: Option<Msg>) -> button::Button<'_, Msg> {
    button(container(text(label).size(12)).center_x(Fill))
        .padding([4, 8])
        .style(chrome::control)
        .on_press_maybe(message)
}

/// A button that stays pressed while its option is selected.
fn choice(label: &str, active: bool, message: Option<Msg>) -> button::Button<'_, Msg> {
    button(text(label).size(12))
        .padding([4, 8])
        .style(move |theme, status| {
            chrome::control(
                theme,
                if active {
                    button::Status::Pressed
                } else {
                    status
                },
            )
        })
        .on_press_maybe(message)
}

fn toggle(label: &str, value: bool, message: fn(bool) -> Msg) -> Element<'_, Msg> {
    checkbox(value)
        .label(label)
        .on_toggle(message)
        .size(14)
        .spacing(6)
        .text_size(12)
        .into()
}

/// The rotatable, animated player model.
struct Viewer<'a>(&'a Editor);

struct ViewerState {
    yaw: f32,
    pitch: f32,
    drag: Option<Point>,
    /// The last rendered frame and what it was rendered from.
    frame: RefCell<Option<([u32; 4], u64, image::Handle)>>,
}
impl Default for ViewerState {
    fn default() -> Self {
        Self {
            yaw: 0.5,
            pitch: -0.15,
            drag: None,
            frame: RefCell::new(None),
        }
    }
}

impl canvas::Program<Msg> for Viewer<'_> {
    type State = ViewerState;

    fn update(
        &self,
        state: &mut ViewerState,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Msg>> {
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                state.drag = cursor.position_over(bounds);
                state.drag.map(|_| canvas::Action::capture())
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.drag = None;
                None
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                // Only a drag that began on the preview turns the model.
                let last = state.drag.as_mut()?;
                let last = std::mem::replace(last, *position);
                state.yaw += (position.x - last.x) * 0.012;
                state.pitch = (state.pitch - (position.y - last.y) * 0.012).clamp(-1.3, 1.3);
                Some(canvas::Action::request_redraw().and_capture())
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &ViewerState,
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let editor = self.0;
        let moving = editor.animating() || state.drag.is_some();
        let (width, height, scale) = frame_size(bounds.size(), moving);
        let key = [width, height, state.yaw.to_bits(), state.pitch.to_bits()];
        let mut cached = state.frame.borrow_mut();
        let handle = match &*cached {
            Some((cached_key, revision, handle))
                if *cached_key == key && *revision == editor.revision =>
            {
                handle.clone()
            }
            _ => {
                let scene = Scene {
                    skin: &editor.skin,
                    cape: editor.cape.as_ref(),
                    options: Options {
                        slim: editor.slim,
                        overlay: editor.overlay,
                    },
                    motion: Motion {
                        walk: editor.walk,
                        phase: editor.phase,
                        amount: editor.amount,
                        age: editor.age,
                    },
                    yaw: state.yaw,
                    pitch: state.pitch,
                };
                let pixels = skin_model::render(
                    &scene,
                    width as usize,
                    height as usize,
                    // The Update Notes stone.
                    &skin_model::Backdrop {
                        tile: crate::news::TILE,
                        size: crate::news::TILE_SIZE as usize,
                        zoom: scale,
                    },
                );
                let handle = image::Handle::from_rgba(width, height, pixels);
                *cached = Some((key, editor.revision, handle.clone()));
                handle
            }
        };
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.draw_image(
            Rectangle::with_size(bounds.size()),
            canvas::Image::new(handle).filter_method(image::FilterMethod::Linear),
        );
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &ViewerState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.drag.is_some() {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(bounds) {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }
}

/// iced uploads images of 2 MiB or more on a worker thread and draws nothing
/// until that finishes, which made a large preview blink on every new frame.
const MAX_FRAME_BYTES: f32 = 2. * 1024. * 1024. - 4096.;

/// The preview's render size in pixels and its scale over the widget's size.
///
/// Moving frames are drawn at the widget's size; once the model rests it is
/// redrawn at twice that, which keeps texel edges sharp on dense displays. Both
/// shrink as needed to stay within what iced uploads immediately.
fn frame_size(area: Size, moving: bool) -> (u32, u32, f32) {
    let wanted: f32 = if moving { 1. } else { 2. };
    let limit = (MAX_FRAME_BYTES / 4. / (area.width * area.height).max(1.)).sqrt();
    let scale = wanted.min(limit);
    (
        ((area.width * scale) as u32).max(1),
        ((area.height * scale) as u32).max(1),
        scale,
    )
}

/// Where the texture sits inside the paint area: its top-left corner and texel size.
fn placement(area: Size, height: usize) -> (Point, f32) {
    let cell = (area.width / SIZE as f32)
        .min(area.height / height as f32)
        .max(1.);
    (
        Point::new(
            ((area.width - cell * SIZE as f32) / 2.).max(0.).floor(),
            ((area.height - cell * height as f32) / 2.).max(0.).floor(),
        ),
        cell,
    )
}

/// The texture itself, below the grid (canvas images always draw above paths).
struct Sheet<'a> {
    image: &'a image::Handle,
    height: usize,
}

impl canvas::Program<Msg> for Sheet<'_> {
    type State = ();

    fn draw(
        &self,
        (): &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let (origin, cell) = placement(bounds.size(), self.height);
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.draw_image(
            Rectangle::new(
                origin,
                Size::new(cell * SIZE as f32, cell * self.height as f32),
            ),
            canvas::Image::new(self.image.clone()).filter_method(image::FilterMethod::Nearest),
        );
        vec![frame.into_geometry()]
    }
}

/// Grid lines, body-part guides, the hovered texel and all painting input.
struct Grid {
    height: usize,
    guides: Vec<[usize; 4]>,
}

#[derive(Default)]
struct GridState {
    hover: Option<(usize, usize)>,
    painting: bool,
}

impl Grid {
    fn texel(&self, bounds: Rectangle, cursor: mouse::Cursor) -> Option<(usize, usize)> {
        let position = cursor.position_in(bounds)?;
        let (origin, cell) = placement(bounds.size(), self.height);
        let x = ((position.x - origin.x) / cell).floor();
        let y = ((position.y - origin.y) / cell).floor();
        (x >= 0. && y >= 0. && x < SIZE as f32 && y < self.height as f32)
            .then_some((x as usize, y as usize))
    }
}

impl canvas::Program<Msg> for Grid {
    type State = GridState;

    fn update(
        &self,
        state: &mut GridState,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Msg>> {
        let canvas::Event::Mouse(event) = event else {
            return None;
        };
        let texel = self.texel(bounds, cursor);
        match event {
            mouse::Event::ButtonPressed(mouse::Button::Left) => {
                let (x, y) = texel?;
                state.painting = true;
                Some(canvas::Action::publish(Msg::Paint { x, y, start: true }).and_capture())
            }
            mouse::Event::ButtonPressed(mouse::Button::Right) => {
                let (x, y) = texel?;
                Some(canvas::Action::publish(Msg::Pick { x, y }).and_capture())
            }
            mouse::Event::ButtonReleased(mouse::Button::Left) if state.painting => {
                state.painting = false;
                Some(canvas::Action::publish(Msg::StrokeEnd))
            }
            mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft if state.hover != texel => {
                state.hover = texel;
                match texel {
                    Some((x, y)) if state.painting => {
                        Some(canvas::Action::publish(Msg::Paint { x, y, start: false }))
                    }
                    _ => Some(canvas::Action::request_redraw()),
                }
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &GridState,
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let (origin, cell) = placement(bounds.size(), self.height);
        let at =
            |x: usize, y: usize| Point::new(origin.x + x as f32 * cell, origin.y + y as f32 * cell);
        let line = |shade: f32, alpha: f32| {
            canvas::Stroke::default()
                .with_width(1.)
                .with_color(Color::from_rgba(shade, shade, shade, alpha))
        };
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        if cell >= 6. {
            let grid = canvas::Path::new(|path| {
                for x in 0..=SIZE {
                    path.move_to(at(x, 0));
                    path.line_to(at(x, self.height));
                }
                for y in 0..=self.height {
                    path.move_to(at(0, y));
                    path.line_to(at(SIZE, y));
                }
            });
            frame.stroke(&grid, line(0., 0.12));
        }
        for &[x, y, w, h] in &self.guides {
            let area =
                canvas::Path::rectangle(at(x, y), Size::new(w as f32 * cell, h as f32 * cell));
            frame.stroke(
                &area,
                canvas::Stroke::default()
                    .with_width(1.)
                    .with_color(Color::from_rgba8(0x00, 0x78, 0xd7, 0.75)),
            );
        }
        if let Some((x, y)) = state.hover {
            let texel = canvas::Path::rectangle(at(x, y), Size::new(cell, cell));
            frame.stroke(&texel, line(0., 1.).with_width(3.));
            frame.stroke(&texel, line(1., 1.));
        }
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _: &GridState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if self.texel(bounds, cursor).is_some() {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

/// The visible rows at two pixels per texel, blended over a checkerboard so
/// transparent texels read as such.
fn sheet(texture: &Texture) -> image::Handle {
    let (width, height) = (SIZE * 2, texture.height * 2);
    let mut pixels = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let [r, g, b, a] = texture.get(x / 2, y / 2);
            let board = if (x + y) % 2 == 0 { 0xff } else { 0xd8 };
            let alpha = u32::from(a);
            pixels.extend(
                [r, g, b]
                    .map(|value| ((u32::from(value) * alpha + board * (255 - alpha)) / 255) as u8),
            );
            pixels.push(255);
        }
    }
    image::Handle::from_rgba(width as u32, height as u32, pixels)
}

/// A thumbnail of the face with its hat layer.
fn face(skin: &Texture) -> image::Handle {
    let mut pixels = Vec::with_capacity(8 * 8 * 4);
    for y in 8..16 {
        for x in 8..16 {
            let hat = skin.get(x + 32, y);
            pixels.extend(if hat[3] >= 128 { hat } else { skin.get(x, y) });
        }
    }
    image::Handle::from_rgba(8, 8, pixels)
}

/// A thumbnail of the outside of a cloak.
fn cape_front(cape: &Texture) -> image::Handle {
    let mut pixels = Vec::with_capacity(10 * 16 * 4);
    for y in 1..17 {
        for x in 1..11 {
            pixels.extend(cape.get(x, y));
        }
    }
    image::Handle::from_rgba(10, 16, pixels)
}

/// Sixteen swatches from the texture: its most used colors, preferring ones
/// that look different from each other so shades of one color do not crowd
/// out the rest. Sparse textures are topped up from the stock palette.
fn palette(texture: &Texture) -> Vec<[u8; 3]> {
    let mut counts = std::collections::HashMap::<[u8; 3], u32>::new();
    for &[r, g, b, a] in &texture.pixels[..SIZE * texture.height] {
        if a >= 128 {
            *counts.entry([r, g, b]).or_default() += 1;
        }
    }
    let mut used: Vec<_> = counts.into_iter().collect();
    used.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let stock = PALETTE.map(|value| [(value >> 16) as u8, (value >> 8) as u8, value as u8]);
    let candidates: Vec<[u8; 3]> = used
        .into_iter()
        .map(|(color, _)| color)
        .chain(stock)
        .collect();
    let distance = |a: [u8; 3], b: [u8; 3]| {
        (0..3)
            .map(|i| (i32::from(a[i]) - i32::from(b[i])).pow(2))
            .sum::<i32>()
    };
    let mut swatches: Vec<[u8; 3]> = Vec::with_capacity(PALETTE.len());
    for apart in [56_i32, 28, 1] {
        for &color in &candidates {
            if swatches.len() == PALETTE.len() {
                return swatches;
            }
            if swatches
                .iter()
                .all(|&chosen| distance(chosen, color) >= apart * apart)
            {
                swatches.push(color);
            }
        }
    }
    swatches
}

fn flood(texture: &mut Texture, x: usize, y: usize, ink: [u8; 4]) {
    let from = texture.get(x, y);
    if from == ink {
        return;
    }
    let mut pending = vec![(x, y)];
    while let Some((x, y)) = pending.pop() {
        if texture.get(x, y) != from {
            continue;
        }
        texture.set(x, y, ink);
        if x > 0 {
            pending.push((x - 1, y));
        }
        if x + 1 < SIZE {
            pending.push((x + 1, y));
        }
        if y > 0 {
            pending.push((x, y - 1));
        }
        if y + 1 < texture.height {
            pending.push((x, y + 1));
        }
    }
}

/// A plain placeholder character in the legacy 64 by 32 layout.
fn default_skin() -> Texture {
    const SKIN: [u8; 4] = [0xc6, 0x96, 0x6c, 255];
    const HAIR: [u8; 4] = [0x3a, 0x28, 0x1c, 255];
    const SHIRT: [u8; 4] = [0xb7, 0x41, 0x0e, 255];
    const TROUSERS: [u8; 4] = [0x3c, 0x44, 0x50, 255];
    const SHOES: [u8; 4] = [0x2a, 0x2a, 0x2e, 255];
    let mut skin = Texture::new(32);
    // Head: hair on top, down the back and along the brow.
    skin.fill([0, 8, 32, 8], SKIN);
    skin.fill([8, 0, 8, 8], HAIR);
    skin.fill([16, 0, 8, 8], SKIN);
    skin.fill([0, 8, 32, 2], HAIR);
    skin.fill([24, 8, 8, 8], HAIR);
    for x in [9, 13] {
        skin.fill([x, 12, 2, 1], [255, 255, 255, 255]);
    }
    for x in [10, 13] {
        skin.set(x, 12, [0x2b, 0x4a, 0x7a, 255]);
    }
    skin.fill([11, 14, 2, 1], [0x8a, 0x5a, 0x44, 255]);
    // Body.
    skin.fill([20, 16, 16, 4], SHIRT);
    skin.fill([16, 20, 24, 12], SHIRT);
    // Arm: short sleeve over bare skin.
    skin.fill([44, 16, 4, 4], SHIRT);
    skin.fill([48, 16, 4, 4], SKIN);
    skin.fill([40, 20, 16, 12], SKIN);
    skin.fill([40, 20, 16, 4], SHIRT);
    // Leg.
    skin.fill([4, 16, 8, 4], TROUSERS);
    skin.fill([0, 20, 16, 12], TROUSERS);
    skin.fill([0, 30, 16, 2], SHOES);
    skin.fill([8, 16, 4, 4], SHOES);
    skin
}

/// Reads a skin or cloak PNG into the 64-pixel layout, as the Oxide client does.
/// Returns the texture and whether a 64 by 64 skin has slim arms.
pub(crate) fn decode(bytes: &[u8], cape: bool) -> Result<(Texture, bool), String> {
    if bytes.len() > MAX_PNG {
        return Err("The file is larger than 1 MiB".into());
    }
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder
        .read_info()
        .map_err(|error| format!("Not a PNG image: {error}"))?;
    let (width, height) = (reader.info().width as usize, reader.info().height as usize);
    if width > 1024 || height > 1024 {
        return Err("The image is larger than 1024 pixels".into());
    }
    let mut data = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or("The image is too large")?
    ];
    let frame = reader
        .next_frame(&mut data)
        .map_err(|error| format!("Could not decode the PNG: {error}"))?;
    let channels = match frame.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return Err("Unsupported PNG palette".into()),
    };
    let pixel = |x: usize, y: usize| {
        let p = &data[y * frame.line_size + x * channels..][..channels];
        match channels {
            1 => [p[0], p[0], p[0], 255],
            2 => [p[0], p[0], p[0], p[1]],
            3 => [p[0], p[1], p[2], 255],
            _ => [p[0], p[1], p[2], p[3]],
        }
    };
    // Higher-resolution textures are reduced to their 64-pixel layout.
    let reduce = |columns: usize, rows: usize, layout: usize| {
        let scale = width / columns;
        let mut texture = Texture::new(layout);
        for y in 0..rows {
            for x in 0..columns {
                texture.set(x, y, pixel(x * scale + scale / 2, y * scale + scale / 2));
            }
        }
        texture
    };
    // Cropped cloaks keep their native 22 by 17 rectangle.
    if cape && width >= 22 && width.is_multiple_of(22) && height == 17 * (width / 22) {
        return Ok((reduce(22, 17, 32), false));
    }
    let modern = !cape && height == width;
    if width < SIZE || !width.is_multiple_of(SIZE) || !(height == width / 2 || modern) {
        return Err(if cape {
            "Capes must be 64×32 or 22×17 (or a multiple)".into()
        } else {
            "Skins must be 64×32 or 64×64 (or a multiple)".into()
        });
    }
    let rows = if modern { SIZE } else { 32 };
    let mut texture = reduce(SIZE, rows, rows);
    if cape {
        return Ok((texture, false));
    }
    let transparent = |texture: &Texture, [x, y, w, h]: [usize; 4]| {
        (y..y + h).all(|y| (x..x + w).all(|x| texture.get(x, y)[3] == 0))
    };
    // Slim skins leave two columns unused on each arm's base layer.
    let slim = modern
        && [(40, 20, 54), (32, 52, 46)]
            .into_iter()
            .all(|(start, y, unused)| {
                transparent(&texture, [unused, y, 2, 12])
                    && !transparent(&texture, [start, y, unused - start, 12])
            });
    // Legacy skins often fill the unused hat area solid; the game hides it.
    if !modern && (0..16).all(|y| (32..SIZE).all(|x| texture.get(x, y)[3] >= 128)) {
        texture.fill([32, 0, 32, 16], [0; 4]);
    }
    Ok((texture, slim))
}

pub(crate) fn encode(texture: &Texture) -> Result<Vec<u8>, String> {
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, SIZE as u32, texture.height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(&texture.rgba()))
        .map_err(|error| error.to_string())?;
    Ok(png)
}

/// A file stem that is safe inside the cape library on every platform.
fn library_name(name: &str) -> String {
    let stem = name.strip_suffix(".png").unwrap_or(name);
    let stem = stem.strip_suffix(".PNG").unwrap_or(stem);
    let clean: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || " -_()".contains(c) {
                c
            } else {
                '_'
            }
        })
        .take(40)
        .collect();
    let clean = clean.trim();
    if clean.is_empty() {
        "cape".into()
    } else {
        clean.into()
    }
}

async fn pick_png(title: &'static str) -> Picked {
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_title(title)
        .add_filter("PNG image", &["png"])
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    let name = file.file_name();
    let size = tokio::fs::metadata(file.path())
        .await
        .map_err(|error| format!("Could not read {name}: {error}"))?
        .len();
    if size > MAX_PNG as u64 {
        return Err(format!("{name} is larger than 1 MiB."));
    }
    Ok(Some((name, file.read().await)))
}

async fn export(name: &'static str, png: Vec<u8>) -> Result<Option<PathBuf>, String> {
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_title("Export PNG")
        .set_file_name(name)
        .add_filter("PNG image", &["png"])
        .save_file()
        .await
    else {
        return Ok(None);
    };
    file.write(&png).await.map_err(|error| error.to_string())?;
    Ok(Some(file.path().to_path_buf()))
}

/// A GET that treats "no such thing" as `None` and bounds the answer's size.
async fn download(client: &reqwest::Client, url: &str) -> Result<Option<Vec<u8>>, String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if matches!(
        response.status(),
        reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::NO_CONTENT
    ) {
        return Ok(None);
    }
    let bytes = response
        .error_for_status()
        .map_err(|error| error.to_string())?
        .bytes()
        .await
        .map_err(|error| error.to_string())?;
    if bytes.len() > MAX_PNG {
        return Err("The service sent an oversized answer".into());
    }
    Ok(Some(bytes.to_vec()))
}

/// The skin and cloak the Oxide client shows first: Old School Minecraft's own.
async fn osm(client: &reqwest::Client, name: &str) -> Result<Cosmetics, String> {
    let skin = format!("{COSMETICS}/skin/{name}");
    let cape = format!("{COSMETICS}/cloak/{name}");
    let (skin, cape) = futures_util::join!(download(client, &skin), download(client, &cape));
    Ok(Cosmetics {
        skin: skin?,
        cape: cape?,
        slim: None,
    })
}

/// Only Mojang's own texture host is fetched, always over HTTPS.
fn mojang_texture(url: &str) -> Result<String, String> {
    const HOST: &str = "textures.minecraft.net/texture/";
    url.strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .and_then(|rest| rest.strip_prefix(HOST))
        .filter(|hash| {
            (32..=64).contains(&hash.len()) && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        .map(|hash| format!("https://{HOST}{hash}"))
        .ok_or_else(|| "Mojang sent an unexpected texture address".to_owned())
}

/// The skin, cape and arm model in a Mojang session profile.
fn mojang_textures(profile: &[u8]) -> Result<(Option<String>, Option<String>, bool), String> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let invalid = || "Mojang sent an unreadable profile".to_owned();
    let profile: serde_json::Value = serde_json::from_slice(profile).map_err(|_| invalid())?;
    let encoded = profile["properties"]
        .as_array()
        .and_then(|properties| properties.iter().find(|entry| entry["name"] == "textures"))
        .and_then(|entry| entry["value"].as_str())
        .ok_or_else(invalid)?;
    let textures: serde_json::Value = STANDARD
        .decode(encoded)
        .ok()
        .and_then(|json| serde_json::from_slice(&json).ok())
        .ok_or_else(invalid)?;
    let address = |kind: &str| {
        textures["textures"][kind]["url"]
            .as_str()
            .map(mojang_texture)
            .transpose()
    };
    Ok((
        address("SKIN")?,
        address("CAPE")?,
        textures["textures"]["SKIN"]["metadata"]["model"] == "slim",
    ))
}

/// The skin and cape of the Mojang account with this username, if there is one.
async fn mojang(client: &reqwest::Client, name: &str) -> Result<Cosmetics, String> {
    let lookup = format!("https://api.mojang.com/users/profiles/minecraft/{name}");
    let Some(user) = download(client, &lookup).await? else {
        return Ok(Cosmetics::default());
    };
    let user: serde_json::Value =
        serde_json::from_slice(&user).map_err(|_| "Mojang sent an unreadable answer")?;
    let id = user["id"]
        .as_str()
        .filter(|id| id.len() == 32 && id.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or("Mojang sent an unexpected account id")?;
    let session = format!("https://sessionserver.mojang.com/session/minecraft/profile/{id}");
    let Some(profile) = download(client, &session).await? else {
        return Ok(Cosmetics::default());
    };
    let (skin, cape, slim) = mojang_textures(&profile)?;
    let fetch = async |address: Option<String>| match address {
        Some(address) => download(client, &address).await,
        None => Ok(None),
    };
    let (skin, cape) = futures_util::join!(fetch(skin), fetch(cape));
    Ok(Cosmetics {
        skin: skin?,
        cape: cape?,
        slim: Some(slim),
    })
}

/// What `source` has for the username.
async fn account(name: String, source: SkinSource) -> Account {
    let cosmetics = async {
        let client = reqwest::Client::builder()
            .user_agent(concat!("Oxide-Launcher/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(8))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| error.to_string())?;
        match source {
            SkinSource::Osm => osm(&client, &name).await,
            SkinSource::Mojang => mojang(&client, &name).await,
        }
    }
    .await;
    Account {
        cosmetics,
        name,
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&color.repeat((width * height) as usize))
            .unwrap();
        drop(writer);
        out
    }

    #[test]
    fn skins_and_capes_decode_to_the_client_layouts() {
        let (legacy, slim) = decode(&png(64, 32, [9, 8, 7, 255]), false).unwrap();
        assert_eq!((legacy.height, slim), (32, false));
        assert_eq!(legacy.get(0, 0), [9, 8, 7, 255]);
        // A solid legacy hat area is hidden, as in the game.
        assert_eq!(legacy.get(40, 4), [0; 4]);
        let (modern, _) = decode(&png(128, 128, [1, 2, 3, 255]), false).unwrap();
        assert_eq!(modern.height, 64);
        assert_eq!(modern.get(63, 63), [1, 2, 3, 255]);
        let (cropped, _) = decode(&png(44, 34, [5, 5, 5, 255]), true).unwrap();
        assert_eq!(cropped.get(21, 16), [5, 5, 5, 255]);
        assert_eq!(cropped.get(22, 0), [0; 4]);
        assert!(decode(&png(64, 64, [0; 4]), true).is_err());
        assert!(decode(&png(50, 50, [0; 4]), false).is_err());
        assert!(decode(b"not a png", false).is_err());
    }

    #[test]
    fn exported_skins_reload_unchanged() {
        let skin = default_skin();
        let (reloaded, slim) = decode(&encode(&skin).unwrap(), false).unwrap();
        assert!(reloaded == skin && !slim);
    }

    #[test]
    fn painting_is_undoable_and_drafts_survive_a_restart() {
        let root = tempfile::tempdir().unwrap();
        let mut editor = Editor::new(Some(root.path()));
        let original = editor.skin.clone();
        let _ = editor.update(Msg::Color([1, 2, 3]), None);
        let _ = editor.update(
            Msg::Paint {
                x: 2,
                y: 20,
                start: true,
            },
            None,
        );
        let _ = editor.update(
            Msg::Paint {
                x: 6,
                y: 24,
                start: false,
            },
            None,
        );
        let _ = editor.update(Msg::StrokeEnd, None);
        // The drag is joined into a continuous line.
        for step in 2..=6 {
            assert_eq!(editor.skin.get(step, step + 18), [1, 2, 3, 255]);
        }
        let painted = editor.skin.clone();
        assert!(Editor::new(Some(root.path())).skin == painted);
        let _ = editor.update(Msg::Undo, None);
        assert!(editor.skin == original);
        let _ = editor.update(Msg::Redo, None);
        assert!(editor.skin == painted);
        let _ = editor.update(Msg::Tool(Tool::Eraser), None);
        let _ = editor.update(
            Msg::Paint {
                x: 2,
                y: 20,
                start: true,
            },
            None,
        );
        assert_eq!(editor.skin.get(2, 20), [0; 4]);
        let _ = editor.update(Msg::Pick { x: 3, y: 21 }, None);
        assert_eq!(editor.hex, "#010203");
    }

    #[test]
    fn uploaded_capes_join_the_library_and_can_be_reselected() {
        let root = tempfile::tempdir().unwrap();
        let mut editor = Editor::new(Some(root.path()));
        let upload = Ok(Some(("My Cape!.png".into(), png(22, 17, [7, 7, 7, 255]))));
        let _ = editor.update(Msg::CapeUploaded(upload), None);
        assert_eq!(editor.library, ["My Cape_"]);
        assert_eq!(editor.cape_choice, CapeChoice::Library("My Cape_".into()));
        let _ = editor.update(Msg::Target(Target::Cape), None);
        assert_eq!(editor.target, Target::Cape);
        let _ = editor.update(Msg::Cape(CapeChoice::None), None);
        assert!(editor.cape.is_none());
        assert_eq!(editor.target, Target::Skin);
        let _ = editor.update(Msg::Cape(CapeChoice::Library("My Cape_".into())), None);
        assert_eq!(editor.cape.as_ref().unwrap().get(0, 0), [7, 7, 7, 255]);
        let bad = Ok(Some(("bad.png".into(), png(30, 30, [0; 4]))));
        let _ = editor.update(Msg::CapeUploaded(bad), None);
        assert_eq!(editor.library.len(), 1);
        assert!(editor.status.contains("bad.png"));
    }

    #[test]
    fn save_is_a_placeholder_and_walking_eases_in_and_out() {
        let mut editor = Editor::new(None);
        let skin = editor.skin.clone();
        let _ = editor.update(Msg::Save, Some("Alex".into()));
        assert!(editor.skin == skin && editor.status.contains("isn't available yet"));
        assert!(!editor.animating());
        let _ = editor.update(Msg::TogglePlay, None);
        let start = Instant::now();
        for frame in 0..40 {
            let _ = editor.update(Msg::Frame(start + Duration::from_millis(50 * frame)), None);
        }
        assert!(editor.phase > 10. && (editor.amount - WALK_AMOUNT).abs() < 0.01);
        let _ = editor.update(Msg::TogglePlay, None);
        for frame in 40..90 {
            let _ = editor.update(Msg::Frame(start + Duration::from_millis(50 * frame)), None);
        }
        assert!(!editor.animating());
    }
}

#[cfg(test)]
mod lookup_tests {
    use super::*;

    fn png(color: [u8; 4]) -> Vec<u8> {
        let mut skin = Texture::new(32);
        skin.fill([0, 0, SIZE, 32], color);
        encode(&skin).unwrap()
    }

    fn answer(name: &str, skin: Option<[u8; 4]>) -> Msg {
        Msg::AccountLoaded(Account {
            name: name.into(),
            source: SkinSource::Osm,
            cosmetics: Ok(Cosmetics {
                skin: skin.map(png),
                ..Default::default()
            }),
        })
    }

    #[test]
    fn history_keeps_loaded_and_replaced_skins_across_restarts() {
        let root = tempfile::tempdir().unwrap();
        let mut editor = Editor::new(Some(root.path()));
        let _ = editor.start(Some("Alex"));
        let _ = editor.update(answer("Alex", Some([2, 2, 2, 255])), None);
        assert_eq!(editor.history.len(), 1);
        assert_eq!(editor.history[0].name, "Alex (OSM)");
        // Painting, then loading another skin, keeps the painted one reachable.
        let _ = editor.update(
            Msg::Paint {
                x: 1,
                y: 20,
                start: true,
            },
            None,
        );
        let _ = editor.update(Msg::StrokeEnd, None);
        let painted = editor.skin.clone();
        let file = Ok(Some(("mine.png".into(), png([9, 9, 9, 255]))));
        let _ = editor.update(Msg::SkinOpened(file), None);
        let names: Vec<_> = editor
            .history
            .iter()
            .map(|saved| saved.name.as_str())
            .collect();
        assert_eq!(names, ["mine", "Edited skin", "Alex (OSM)"]);
        // Loading the same skin again does not list it twice.
        let again = Ok(Some(("copy.png".into(), png([9, 9, 9, 255]))));
        let _ = editor.update(Msg::SkinOpened(again), None);
        assert_eq!(editor.history.len(), 3);
        assert_eq!(editor.history[0].name, "copy");
        let _ = editor.update(Msg::History(1), None);
        assert!(editor.skin == painted);
        let reopened = Editor::new(Some(root.path()));
        let names: Vec<_> = reopened
            .history
            .iter()
            .map(|saved| saved.name.as_str())
            .collect();
        assert_eq!(names, ["copy", "Edited skin", "Alex (OSM)"]);
        assert!(reopened.history[1].texture == painted);
    }

    #[test]
    fn the_source_is_followed_and_states_the_arm_model() {
        let mut editor = Editor::new(None).with_source(SkinSource::Mojang);
        let _ = editor.start(Some("Alex"));
        // An answer from the source that was selected earlier is stale.
        let _ = editor.update(answer("Alex", Some([2, 2, 2, 255])), None);
        assert_eq!(editor.loading.as_deref(), Some("Alex"));
        let mut modern = Texture::new(SIZE);
        modern.fill([0, 0, SIZE, SIZE], [3, 3, 3, 255]);
        let _ = editor.update(
            Msg::AccountLoaded(Account {
                name: "Alex".into(),
                source: SkinSource::Mojang,
                cosmetics: Ok(Cosmetics {
                    skin: Some(encode(&modern).unwrap()),
                    cape: None,
                    slim: Some(true),
                }),
            }),
            None,
        );
        assert!(editor.slim && editor.history[0].name == "Alex (Mojang)");
        let _ = editor.update(Msg::Source(SkinSource::Osm), None);
        assert_eq!(
            (editor.source, editor.loading.as_deref()),
            (SkinSource::Osm, Some("Alex"))
        );
    }

    #[test]
    fn the_username_box_drives_the_skin_and_stale_answers_are_dropped() {
        let root = tempfile::tempdir().unwrap();
        let mut editor = Editor::new(Some(root.path()));
        let _ = editor.start(Some("Alex"));
        assert_eq!(editor.loading.as_deref(), Some("Alex"));
        let _ = editor.username_changed(Some("Alex"));
        let _ = editor.username_changed(Some("Alexa"));
        // A paused-on name that is no longer in the box is not fetched.
        let _ = editor.update(Msg::Lookup("Alex".into()), None);
        let _ = editor.update(Msg::Lookup("Alexa".into()), None);
        assert_eq!(editor.loading.as_deref(), Some("Alexa"));
        let before = editor.skin.clone();
        let _ = editor.update(answer("Alex", Some([1, 1, 1, 255])), None);
        assert!(editor.skin == before);
        let _ = editor.update(answer("Alexa", Some([2, 2, 2, 255])), None);
        assert_eq!(editor.skin.get(0, 20), [2, 2, 2, 255]);
        // An account skin is not a local draft, so the next start refreshes it.
        assert!(!editor.edited && !root.path().join("skin-editor/skin.png").exists());
        let _ = editor.update(Msg::Undo, None);
        assert!(editor.skin == before);
    }

    #[test]
    fn edited_drafts_are_not_replaced_on_start_or_by_skinless_accounts() {
        let root = tempfile::tempdir().unwrap();
        let mut editor = Editor::new(Some(root.path()));
        let _ = editor.update(
            Msg::Paint {
                x: 1,
                y: 20,
                start: true,
            },
            None,
        );
        let _ = editor.update(Msg::StrokeEnd, None);
        let painted = editor.skin.clone();
        let mut restarted = Editor::new(Some(root.path()));
        let _ = restarted.start(Some("Alex"));
        assert!(restarted.loading.is_none() && restarted.skin == painted);
        let _ = restarted.username_changed(Some("Steve"));
        let _ = restarted.update(Msg::Lookup("Steve".into()), None);
        let _ = restarted.update(answer("Steve", None), None);
        assert!(restarted.skin == painted && restarted.status.contains("kept"));
    }
}

#[cfg(test)]
mod palette_tests {
    use super::*;

    #[test]
    fn swatches_come_from_the_skin_and_follow_finished_strokes() {
        let mut skin = Texture::new(32);
        skin.fill([0, 0, SIZE, 32], [10, 20, 30, 255]);
        skin.fill([0, 0, 8, 8], [200, 0, 0, 255]);
        // A near-identical shade and a transparent color do not earn swatches.
        skin.fill([8, 0, 4, 4], [201, 1, 1, 255]);
        skin.fill([16, 0, 4, 4], [0, 255, 0, 0]);
        let swatches = palette(&skin);
        assert_eq!(swatches.len(), 16);
        assert_eq!(swatches[..2], [[10, 20, 30], [200, 0, 0]]);
        assert!(!swatches.contains(&[0, 255, 0]));
        // With room to spare the close shade is still offered, after the stock colors.
        assert!(
            swatches
                .iter()
                .position(|c| *c == [201, 1, 1])
                .is_none_or(|i| i > 2)
        );

        let mut editor = Editor::new(None);
        let ink = [1, 254, 3];
        assert!(!editor.palette.contains(&ink));
        let _ = editor.update(Msg::Color(ink), None);
        let _ = editor.update(Msg::Tool(Tool::Fill), None);
        let _ = editor.update(
            Msg::Paint {
                x: 20,
                y: 24,
                start: true,
            },
            None,
        );
        assert!(!editor.palette.contains(&ink));
        let _ = editor.update(Msg::StrokeEnd, None);
        assert!(editor.palette.contains(&ink));
    }
}

#[cfg(test)]
mod frame_tests {
    use super::*;

    #[test]
    fn preview_frames_stay_small_enough_to_upload_immediately() {
        // The default layout renders at full sharpness.
        assert_eq!(frame_size(Size::new(198., 300.), false), (396, 600, 2.));
        assert_eq!(frame_size(Size::new(198., 300.), true), (198, 300, 1.));
        for area in [
            Size::new(198., 900.),
            Size::new(230., 2000.),
            Size::new(1500., 1500.),
        ] {
            for moving in [false, true] {
                let (width, height, scale) = frame_size(area, moving);
                assert!(width * height * 4 < 2 * 1024 * 1024, "{area:?}");
                assert!(scale > 0. && width >= 1 && height >= 1);
            }
        }
    }
}

#[cfg(test)]
mod mojang_tests {
    use super::*;

    #[test]
    fn session_profiles_yield_https_texture_addresses_and_the_arm_model() {
        use base64::{Engine, engine::general_purpose::STANDARD};
        let hash = "a".repeat(64);
        let textures = serde_json::json!({"textures": {
            "SKIN": {"url": format!("http://textures.minecraft.net/texture/{hash}"),
                     "metadata": {"model": "slim"}},
        }});
        let profile = serde_json::json!({"properties": [
            {"name": "textures", "value": STANDARD.encode(textures.to_string())},
        ]});
        let (skin, cape, slim) = mojang_textures(profile.to_string().as_bytes()).unwrap();
        assert_eq!(
            skin.unwrap(),
            format!("https://textures.minecraft.net/texture/{hash}")
        );
        assert!(cape.is_none() && slim);
        assert!(mojang_texture("https://example.com/texture/aaaa").is_err());
        assert!(mojang_texture("https://textures.minecraft.net/texture/../x").is_err());
        assert!(mojang_textures(b"{}").is_err());
    }
}
