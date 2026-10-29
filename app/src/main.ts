import '@swasthya/ui/base.css'
import { mount } from 'svelte'
import { ui } from '@swasthya/ui'
import App from './App.svelte'

ui.start()

// Screenshot harness: ?theme=aurora&mode=dark&motion=calm previews a look without saving it.
const q = new URLSearchParams(location.search)
if (q.has('theme')) ui.theme = q.get('theme')!
if (q.get('mode') === 'light' || q.get('mode') === 'dark') ui.mode = q.get('mode') as 'light' | 'dark'
if (q.get('motion') === 'calm' || q.get('motion') === 'full') ui.motion = q.get('motion') as 'calm' | 'full'

export default mount(App, { target: document.getElementById('app')! })
