import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import App from './App.svelte';
import { app } from './lib/app.svelte';
import { resetPrototypeData, setLatency } from './lib/commands';

beforeEach(() => {
  setLatency(0);
  resetPrototypeData();
  app.setMode('ready');
  app.go({ name: 'overview' });
  app.entryOpen = false;
  app.helpOpen = false;
  app.toasts = [];
});

async function setup() {
  const user = userEvent.setup();
  render(App);
  await screen.findByText('Cash in accounts');
  await screen.findByRole('button', { name: /Wallet/ });
  return user;
}

function closeOpenDialog() {
  document.querySelector('dialog[open]')!.dispatchEvent(new Event('cancel', { cancelable: true }));
  return tick();
}

describe('overview', () => {
  it('shows an unknown balance as unknown, never as ¥0, and labels the total incomplete', async () => {
    await setup();
    const wallet = screen.getByRole('button', { name: /Wallet/ });
    expect(within(wallet).getByText('Unknown', { selector: '[aria-hidden]' })).toBeInTheDocument();
    expect(within(wallet).queryByText('¥0')).toBeNull();
    expect(screen.getByText(/accounts unknown/)).toBeInTheDocument();
    expect(screen.getByText(/not net worth/i)).toBeInTheDocument();
  });

  it('keeps card debt separate from cash', async () => {
    await setup();
    const owed = screen.getByRole('heading', { name: 'Owed on cards' }).closest('article')!;
    expect(within(owed).getByText('¥86,420')).toBeInTheDocument();
  });

  it('renders an actionable empty state for a new install', async () => {
    const user = await setup();
    app.setMode('empty');
    expect(await screen.findByText('No accounts yet')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Go to Accounts' }));
    expect(screen.getByRole('heading', { level: 1, name: 'Accounts' })).toBeInTheDocument();
  });

  it('reports a storage error without implying data changed, and recovers on retry', async () => {
    const user = await setup();
    app.setMode('error');
    const alerts = await screen.findAllByRole('alert');
    expect(alerts[0]).toHaveTextContent(/Nothing has been changed/);
    app.setMode('ready');
    await waitFor(() => expect(screen.queryAllByRole('alert')).toHaveLength(0));
    app.setMode('error');
    const again = await screen.findAllByRole('alert');
    app.setMode('ready');
    await user.click(within(again[0]!).getByRole('button', { name: 'Try again' }));
    await waitFor(() => expect(screen.queryAllByRole('alert')).toHaveLength(0));
  });
});

describe('keyboard navigation', () => {
  it('switches sections with Ctrl+number and moves focus to the page heading', async () => {
    const user = await setup();
    await user.keyboard('{Control>}2{/Control}');
    const h1 = await screen.findByRole('heading', { level: 1, name: 'Accounts' });
    await waitFor(() => expect(h1).toHaveFocus());
    expect(screen.getByRole('link', { name: /Accounts/ })).toHaveAttribute('aria-current', 'page');
  });

  it('opens shortcut help with ? and closes it with Escape', async () => {
    const user = await setup();
    await user.keyboard('?');
    expect(await screen.findByRole('heading', { name: 'Keyboard shortcuts' })).toBeInTheDocument();
    await closeOpenDialog();
    expect(document.querySelector('dialog[open]')).toBeNull();
  });

  it('moves through the account list with arrow keys and filters history', async () => {
    const user = await setup();
    app.go({ name: 'accounts', accountId: null });
    const list = await screen.findByRole('navigation', { name: 'Accounts' });
    const all = await within(list).findByRole('button', { name: /All accounts/ });
    all.focus();
    await user.keyboard('{ArrowDown}');
    expect(within(list).getByRole('button', { name: /Everyday/ })).toHaveFocus();
    await user.keyboard('{Enter}');
    expect(await screen.findByRole('heading', { level: 2, name: 'Everyday' })).toBeInTheDocument();
    await screen.findByText('Supermarket');
    await user.type(screen.getByRole('searchbox'), 'zzz');
    expect(screen.getByText('No entries match these filters.')).toBeInTheDocument();
  });

  it('labels legacy records with an unknown purchase date instead of guessing one', async () => {
    await setup();
    app.go({ name: 'accounts', accountId: 'acc-everyday' });
    const row = (await screen.findByText('Imported: Home centre')).closest('tr')!;
    expect(within(row).getByText('Purchase date unknown')).toBeInTheDocument();
    expect(within(row).getByText(/^Paid 12 Sept? 2026$/)).toBeInTheDocument();
  });
});

describe('quick entry', () => {
  it('opens with Ctrl+N, focuses the amount, validates whole yen, and returns focus on close', async () => {
    const user = await setup();
    const trigger = screen.getByRole('button', { name: /New entry/ });
    trigger.focus();
    await user.keyboard('{Control>}n{/Control}');
    const amount = await screen.findByRole('textbox', { name: /Amount/ });
    expect(amount).toHaveFocus();
    await user.type(amount, '12.5');
    await user.click(screen.getByRole('button', { name: 'Save entry' }));
    expect(amount).toHaveAttribute('aria-invalid', 'true');
    expect(screen.getByText('Use whole yen — no decimals.')).toBeInTheDocument();
    await closeOpenDialog();
    expect(trigger).toHaveFocus();
  });

  it('saves once even if submitted twice, then updates history', async () => {
    const user = await setup();
    setLatency(30);
    app.entryOpen = true;
    await user.type(await screen.findByRole('textbox', { name: /Amount/ }), '1,480');
    await user.type(screen.getByRole('textbox', { name: /Description/ }), 'Bakery');
    await user.click(screen.getByRole('button', { name: 'Save entry' }));
    const saving = screen.getByRole('button', { name: 'Saving…' });
    expect(saving).toBeDisabled();
    await user.click(saving);
    await screen.findByText(/Saved: spending ¥1,480 — Bakery/);
    app.go({ name: 'accounts', accountId: null });
    await waitFor(() => expect(screen.getAllByText('Bakery')).toHaveLength(1));
  });

  it('keeps the form intact after a failed save and succeeds on retry', async () => {
    const user = await setup();
    app.entryOpen = true;
    const amount = await screen.findByRole('textbox', { name: /Amount/ });
    await user.type(amount, '900');
    await user.type(screen.getByRole('textbox', { name: /Description/ }), 'Taxi');
    app.setMode('error');
    await user.click(screen.getByRole('button', { name: 'Save entry' }));
    expect(await screen.findByText('Nothing was saved.')).toBeInTheDocument();
    expect(amount).toHaveValue('900');
    app.setMode('ready');
    const dialog = document.querySelector<HTMLElement>('dialog[open]')!;
    await user.click(within(dialog).getByRole('button', { name: 'Try again' }));
    await screen.findByText(/Saved: spending ¥900 — Taxi/);
  });

  it('treats a transfer as money moved, not spending', async () => {
    const user = await setup();
    app.entryOpen = true;
    await user.click(await screen.findByRole('radio', { name: 'Transfer' }));
    expect(screen.getByText(/not spending/)).toBeInTheDocument();
    expect(screen.queryByRole('combobox', { name: 'Category' })).toBeNull();
    const from = screen.getByRole('combobox', { name: 'From' });
    expect(within(from).queryByRole('option', { name: 'Travel card' })).toBeNull();
  });
});

describe('cash flow report', () => {
  it('offers an equivalent table that labels forecast and incomplete months', async () => {
    const user = await setup();
    app.go({ name: 'cashflow' });
    await screen.findByRole('group', { name: /Monthly cash flow chart/ });
    await user.click(screen.getByRole('button', { name: 'Table' }));
    const table = screen.getByRole('table', { name: 'Monthly cash flow' });
    const rows = within(table).getAllByRole('row').slice(1);
    expect(rows).toHaveLength(9);
    expect(within(rows[3]!).getByText('Incomplete')).toBeInTheDocument();
    expect(within(rows[7]!).getByText('Forecast')).toBeInTheDocument();
  });

  it('lets keyboard users step through months with arrow keys', async () => {
    const user = await setup();
    app.go({ name: 'cashflow' });
    const chart = await screen.findByRole('group', { name: /Monthly cash flow chart/ });
    const months = within(chart).getAllByRole('img');
    months[0]!.focus();
    await user.keyboard('{End}');
    expect(months[8]).toHaveFocus();
    expect(months[8]).toHaveAccessibleName(/Jan 2027, Forecast/);
  });
});
