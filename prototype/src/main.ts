import { mount } from 'svelte';
import App from './App.svelte';
import './lib/tokens.css';
import './lib/base.css';

mount(App, { target: document.getElementById('app')! });
