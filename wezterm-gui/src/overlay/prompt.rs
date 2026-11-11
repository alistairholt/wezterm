use crate::scripting::guiwin::GuiWin;
use config::keyassignment::{KeyAssignment, PromptInputLine};
use mux::termwiztermtab::TermWizTerminal;
use mux_lua::MuxPane;
use std::rc::Rc;
use termwiz::input::{InputEvent, KeyCode, KeyEvent};
use termwiz::lineedit::*;
use termwiz::surface::Change;
use termwiz::terminal::Terminal;

struct PromptHost {
    history: BasicHistory,
}

impl PromptHost {
    fn new() -> Self {
        Self {
            history: BasicHistory::default(),
        }
    }
}

impl LineEditorHost for PromptHost {
    fn history(&mut self) -> &mut dyn History {
        &mut self.history
    }

    fn resolve_action(
        &mut self,
        event: &InputEvent,
        editor: &mut LineEditor<'_>,
    ) -> Option<Action> {
        let (line, _cursor) = editor.get_line_and_cursor();
        match event {
            InputEvent::Key(KeyEvent {
                key: KeyCode::Escape,
                ..
            }) => line.is_empty().then_some(Action::Cancel),
            _ => None,
        }
    }
}

pub fn show_line_prompt_overlay(
    mut term: TermWizTerminal,
    args: PromptInputLine,
    window: GuiWin,
    pane: MuxPane,
) -> anyhow::Result<()> {
    let name = match *args.action {
        KeyAssignment::EmitEvent(id) => id,
        _ => anyhow::bail!(
            "PromptInputLine requires action to be defined by wezterm.action_callback"
        ),
    };

    term.no_grab_mouse_in_raw_mode();
    let mut text = args.description.replace("\r\n", "\n").replace("\n", "\r\n");
    text.push_str("\r\n");
    term.render(&[Change::Text(text)])?;

    let mut host = PromptHost::new();
    let mut editor = LineEditor::new(&mut term);
    editor.set_prompt(&args.prompt);
    let line =
        editor.read_line_with_optional_initial_value(&mut host, args.initial_value.as_deref())?;

    promise::spawn::spawn_into_main_thread(async move {
        trampoline(name, window, pane, line);
        anyhow::Result::<()>::Ok(())
    })
    .detach();

    Ok(())
}

/// Ask for a new title for `tab_id`, starting from its current one,
/// like kitty's set_tab_title. Leaving it empty goes back to the
/// automatic title. Escape on an empty line cancels.
pub fn show_rename_tab_overlay(
    mut term: TermWizTerminal,
    tab_id: mux::tab::TabId,
    current_title: String,
) -> anyhow::Result<()> {
    term.no_grab_mouse_in_raw_mode();
    term.render(&[Change::Text(
        "Rename this tab. Leave it empty to use the automatic title.\r\n".to_string(),
    )])?;

    let mut host = PromptHost::new();
    let mut editor = LineEditor::new(&mut term);
    editor.set_prompt("Tab title: ");
    let line = editor.read_line_with_optional_initial_value(&mut host, Some(&current_title))?;

    if let Some(title) = line {
        promise::spawn::spawn_into_main_thread(async move {
            if let Some(tab) = mux::Mux::get().get_tab(tab_id) {
                tab.set_title(title.trim());
            }
        })
        .detach();
    }

    Ok(())
}

fn trampoline(name: String, window: GuiWin, pane: MuxPane, line: Option<String>) {
    promise::spawn::spawn(async move {
        config::with_lua_config_on_main_thread(move |lua| do_event(lua, name, window, pane, line))
            .await
    })
    .detach();
}

async fn do_event(
    lua: Option<Rc<mlua::Lua>>,
    name: String,
    window: GuiWin,
    pane: MuxPane,
    line: Option<String>,
) -> anyhow::Result<()> {
    if let Some(lua) = lua {
        let args = lua.pack_multi((window, pane, line))?;

        if let Err(err) = config::lua::emit_event(&lua, (name.clone(), args)).await {
            log::error!("while processing {} event: {:#}", name, err);
        }
    }

    Ok(())
}
