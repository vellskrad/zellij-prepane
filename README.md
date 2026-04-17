# prepane (Zellij plugin)

Floating picker plugin for Zellij: reads a manifest of tab layout templates, lets you choose with keyboard or mouse, then opens a new tab using `new_tabs_with_layout`.

## Manifest format

Templates are passed via plugin configuration key `templates_kdl` (no filesystem access required).

KDL entries like:

```kdl
template label="msod" layout_name="msod"
```

`layout_name` is a layout name known to Zellij (built-in or from your `layout_dir`).

## Build

Arch Linux (system toolchain):

```bash
sudo pacman -S --needed rust rust-wasm cargo make git
```

Then build:

```bash
make install
```

Or manually:

```bash
cargo build --release --target wasm32-wasip1
```

```bash
cp target/wasm32-wasip1/release/prepane.wasm ~/.config/zellij/plugins/prepane.wasm
```

## Zellij config

### Create tabs directory (non-default)

```bash
mkdir -p ~/.config/zellij/tabs
```

### Add a sample tab layout

```bash
cp /home/miroforg/personal/SystemCustomization/2.Top_Config/zellij/tabs/msod.kdl ~/.config/zellij/tabs/msod.kdl
```

### Add a sample named layout (required for `layout_name`)

```bash
mkdir -p ~/.config/zellij/layouts
cp /home/miroforg/personal/SystemCustomization/2.Top_Config/zellij/layouts/msod.kdl ~/.config/zellij/layouts/msod.kdl
```

Note: if your `layout_dir` in `~/.config/zellij/config.kdl` is set to a relative path, Zellij may resolve it relative to your current working directory. Using an absolute `layout_dir` is recommended.

### Create the manifest

```bash
cat /home/miroforg/personal/SystemCustomization/2.Top_Config/zellij/tab-templates.kdl
```

### Add plugin alias + config

Add this to `~/.config/zellij/config.kdl` (inside the `plugins { ... }` block):

```kdl
prepane location="file:/home/miroforg/.config/zellij/plugins/prepane.wasm" {
    templates_kdl r#"
template label="msod" layout_name="msod"
"#
}
```

### Bind `Alt+t` in locked mode

Add this to `~/.config/zellij/config.kdl` (inside `keybinds { locked { ... } }`):

```kdl
bind "Alt t" {
    LaunchOrFocusPlugin "prepane" {
        floating true
        move_to_focused_tab true
    }
}
```

## Permissions

The plugin requests:

- `ChangeApplicationState` — create tabs from layout KDL

You must approve the permission prompt once per plugin version.
