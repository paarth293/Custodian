import { CheckMenuItem, Menu, MenuItem, PredefinedMenuItem } from '@tauri-apps/api/menu';
import { call, inTauri } from '../lib/tauri';

export interface MenuState {
  panelOpen: boolean;
  asleep: boolean;
  demo: boolean;
}

export interface MenuActions {
  togglePanel: () => void;
  toggleNap: () => void;
}

/** Shows Cubby's native right-click menu at the cursor. */
export async function showCubbyMenu(state: MenuState, actions: MenuActions): Promise<void> {
  if (!inTauri) return;
  const items = await Promise.all([
    MenuItem.new({
      text: state.panelOpen ? 'Close chat' : 'Talk to Cubby',
      action: actions.togglePanel,
    }),
    MenuItem.new({ text: state.asleep ? 'Wake up' : 'Take a nap', action: actions.toggleNap }),
    CheckMenuItem.new({
      text: 'Preview Special Form (demo)',
      checked: state.demo,
      action: () => void call('set_demo_mode', { enabled: !state.demo }),
    }),
    PredefinedMenuItem.new({ item: 'Separator' }),
    MenuItem.new({ text: 'Hide Cubby', action: () => void call('hide_cubby') }),
    MenuItem.new({ text: 'Quit Custodian', action: () => void call('quit_app') }),
  ]);
  const menu = await Menu.new({ items });
  await menu.popup();
}
