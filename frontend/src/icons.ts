// Official Fluent SVGs, bundled locally. See assets/fluent/README.md for provenance.
import calculator from './assets/fluent/ic_fluent_calculator_24_regular.svg'
import folder from './assets/fluent/ic_fluent_folder_24_regular.svg'
import activity from './assets/fluent/ic_fluent_desktop_pulse_24_regular.svg'
import terminal from './assets/fluent/ic_fluent_window_console_20_regular.svg'
import settings from './assets/fluent/ic_fluent_settings_24_regular.svg'
import apps from './assets/fluent/ic_fluent_apps_24_regular.svg'
import code from './assets/fluent/ic_fluent_code_24_regular.svg'
import devices from './assets/fluent/ic_fluent_developer_board_24_regular.svg'
import search from './assets/fluent/ic_fluent_search_20_regular.svg'
import plus from './assets/fluent/ic_fluent_add_20_regular.svg'
import refresh from './assets/fluent/ic_fluent_arrow_clockwise_20_regular.svg'
import star from './assets/fluent/ic_fluent_star_16_regular.svg'
import starFilled from './assets/fluent/ic_fluent_star_16_filled.svg'
import enter from './assets/fluent/ic_fluent_arrow_enter_left_20_regular.svg'
import arrowUp from './assets/fluent/ic_fluent_arrow_up_16_regular.svg'

export const icons = { calculator, folder, activity, terminal, settings, apps, code, devices, search, plus, refresh, star, starFilled, enter, arrowUp }
export type IconName = keyof typeof icons

export const systemIcons: Readonly<Record<string, IconName>> = {
  'system:calculator': 'calculator',
  'system:explorer': 'folder',
  'system:taskmanager': 'activity',
  'system:terminal': 'terminal',
  'system:control': 'settings',
  'system:apps': 'apps',
  'system:environment': 'code',
  'system:devices': 'devices',
  'system:systeminfo': 'devices',
  'system:computer': 'devices',
  'system:events': 'activity',
  'system:resources': 'activity',
  'system:disks': 'folder',
  'system:cleanup': 'folder',
  'system:optimize': 'folder',
  'system:updates': 'refresh',
  'system:startup': 'apps',
  'system:defaultapps': 'apps',
}
