<!--
  Category management. Rust decides what is allowed: a category with history
  or children can be archived but not deleted or retyped. The dialogs say so
  before the user commits.
-->
<script lang="ts">
  import { app, WriteSession } from '../lib/app.svelte';
  import { archiveCategory, deleteCategory, updateCategory } from '../lib/finance';
  import { createCategory } from '../lib/ledger';
  import { focusFirstInvalid } from '../lib/forms';
  import type { Category } from '../lib/types';
  import Dialog from '../lib/components/Dialog.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import FormStatus from '../lib/components/FormStatus.svelte';
  import StateTag from '../lib/components/StateTag.svelte';

  type Mode = { kind: 'create'; type: Category['type'] } | { kind: 'edit'; category: Category } | { kind: 'archive'; category: Category } | { kind: 'delete'; category: Category };

  let showArchived = $state(false);
  let mode = $state<Mode | null>(null);
  let name = $state('');
  let type = $state<Category['type']>('expense');
  let parentId = $state('');
  let attempted = $state(false);
  const session = new WriteSession();
  let form: HTMLFormElement | undefined = $state();

  const all = $derived(app.categories ?? []);

  function tree(t: Category['type']) {
    const items = all.filter((c) => c.type === t && (showArchived || c.archivedAt === null));
    const rows: { c: Category; depth: number }[] = [];
    const walk = (parent: string | null, depth: number) => {
      for (const c of items.filter((x) => x.parentId === parent).sort((a, b) => a.name.localeCompare(b.name, 'ja'))) {
        rows.push({ c, depth });
        walk(c.id, depth + 1);
      }
    };
    walk(null, 0);
    // Children whose parent is hidden (archived) still appear at the top level.
    for (const c of items) if (!rows.some((r) => r.c.id === c.id)) rows.push({ c, depth: 0 });
    return rows;
  }

  function open(next: Mode) {
    mode = next;
    attempted = false;
    session.reset();
    if (next.kind === 'create') {
      name = '';
      type = next.type;
      parentId = '';
    } else if (next.kind === 'edit') {
      name = next.category.name;
      type = next.category.type;
      parentId = next.category.parentId ?? '';
    }
  }

  const parentOptions = $derived(
    all.filter((c) => c.type === type && c.archivedAt === null && !(mode?.kind === 'edit' && c.id === mode.category.id)),
  );
  const hasChildren = (c: Category) => all.some((x) => x.parentId === c.id);

  async function save(e: Event) {
    e.preventDefault();
    if (!mode) return;
    attempted = true;
    let ok = false;
    if (mode.kind === 'create' || mode.kind === 'edit') {
      if (!name.trim()) return focusFirstInvalid(form);
      const base = { name: name.trim(), type, ...(parentId ? { parentId } : {}) };
      const r = mode.kind === 'create'
        ? await session.run(base, createCategory)
        : await session.run({ categoryId: mode.category.id, ...base }, updateCategory);
      ok = !!r;
      if (r && !r.isReplay) app.toast(mode.kind === 'create' ? `Added ${r.result.name}.` : `Saved ${r.result.name}.`);
    } else if (mode.kind === 'archive') {
      const archived = mode.category.archivedAt === null;
      const r = await session.run({ categoryId: mode.category.id, archived }, archiveCategory);
      ok = !!r;
      if (r && !r.isReplay) app.toast(archived ? `${mode.category.name} archived.` : `${mode.category.name} restored.`);
    } else {
      const r = await session.run({ categoryId: mode.category.id }, deleteCategory);
      ok = !!r;
      if (r && !r.isReplay) app.toast(`${mode.category.name} deleted.`);
    }
    if (ok) mode = null;
  }

  const title = $derived(
    !mode ? '' : mode.kind === 'create' ? 'Add category' : mode.kind === 'edit' ? `Edit ${mode.category.name}` : mode.kind === 'delete' ? `Delete ${mode.category.name}` : mode.category.archivedAt ? `Restore ${mode.category.name}` : `Archive ${mode.category.name}`,
  );
</script>

<header class="page-head">
  <div>
    <h1 tabindex="-1">Categories</h1>
    <p class="muted">Income and spending categories. Used categories are archived rather than deleted so history keeps its labels.</p>
  </div>
  <label class="check"><input type="checkbox" bind:checked={showArchived} /> Show archived</label>
</header>

<div class="cols-2">
  {#each [{ t: 'expense', label: 'Spending' }, { t: 'income', label: 'Income' }] as group (group.t)}
    {@const rows = tree(group.t as Category['type'])}
    <section class="panel" aria-labelledby="cat-{group.t}">
      <div class="panel-head">
        <h2 id="cat-{group.t}">{group.label}</h2>
        <button class="btn small" onclick={() => open({ kind: 'create', type: group.t as Category['type'] })}>Add {group.label.toLowerCase()} category</button>
      </div>
      {#if app.categories === undefined}
        <p class="panel-body muted" role="status">Loading…</p>
      {:else if rows.length === 0}
        <div class="panel-body"><EmptyState title="No {group.label.toLowerCase()} categories" body={group.t === 'income' ? 'Income entries need an income category, such as 給与 / Salary.' : 'Spending entries need a category, such as 食費 / Groceries.'} /></div>
      {:else}
        <ul class="list-plain tree" aria-label="{group.label} categories">
          {#each rows as { c, depth } (c.id)}
            <li style:padding-left="{depth * 20 + 20}px">
              <span class="name">{#if depth}<span class="muted" aria-hidden="true">└ </span>{/if}{c.name}</span>
              {#if c.archivedAt}<StateTag state="neutral" label="Archived" />{/if}
              <span class="row acts">
                <button class="btn ghost small" onclick={() => open({ kind: 'edit', category: c })} aria-label="Edit {c.name}">Edit</button>
                <button class="btn ghost small" onclick={() => open({ kind: 'archive', category: c })} aria-label="{c.archivedAt ? 'Restore' : 'Archive'} {c.name}">{c.archivedAt ? 'Restore' : 'Archive'}</button>
                <button class="btn ghost small" onclick={() => open({ kind: 'delete', category: c })} aria-label="Delete {c.name}">Delete</button>
              </span>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/each}
</div>

<Dialog open={!!mode} {title} width="480px" onclose={() => (mode = null)}>
  {#if mode}
    <form bind:this={form} class="stacked" novalidate onsubmit={save}>
      {#if mode.kind === 'create' || mode.kind === 'edit'}
        <div class="field"><label class="field-label">
          <span>Name</span>
          <input bind:value={name} maxlength="100" data-autofocus aria-invalid={session.fieldMessage('name') || (attempted && !name.trim()) ? 'true' : undefined} /></label>
          {#if session.fieldMessage('name') ?? (attempted && !name.trim())}<span class="error">{session.fieldMessage('name') ?? 'Give the category a name.'}</span>{/if}
        </div>
        <div class="field"><label class="field-label">
          <span>Type</span>
          <select bind:value={type}><option value="expense">Spending</option><option value="income">Income</option></select></label>
          {#if mode.kind === 'edit' && type !== mode.category.type}<span class="hint">A category with history or subcategories cannot change type.</span>{/if}
        </div>
        <div class="field"><label class="field-label">
          <span>Inside <span class="muted">(optional)</span></span>
          <select bind:value={parentId} aria-invalid={session.fieldMessage('parentId') ? 'true' : undefined}>
            <option value="">Top level</option>
            {#each parentOptions as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
          </select></label>
          {#if session.fieldMessage('parentId')}<span class="error">{session.fieldMessage('parentId')}</span>{/if}
        </div>
        {#if mode.kind === 'edit'}<p class="muted small">Renaming changes how past entries are labelled. Their amounts and dates do not change.</p>{/if}
      {:else if mode.kind === 'archive'}
        {#if mode.category.archivedAt === null}
          <div class="notice"><strong>What archiving does</strong><ul>
            <li>Hides {mode.category.name} from new entries and budgets.</li>
            <li>Keeps every past entry and report total in this category.</li>
            {#if hasChildren(mode.category)}<li>Subcategories stay as they are; archive them separately if needed.</li>{/if}
          </ul></div>
        {:else}
          <p class="notice info">Restoring makes {mode.category.name} available for new entries again.</p>
        {/if}
      {:else}
        <div class="notice danger"><strong>Delete permanently?</strong><ul>
          <li>Only a category that has never been used and has no subcategories can be deleted.</li>
          <li>If it has history, archive it instead — nothing is lost that way.</li>
        </ul></div>
      {/if}
      <FormStatus {session} fields={['name', 'parentId']} />
      <div class="row end">
        <button type="button" class="btn" onclick={() => (mode = null)}>Cancel</button>
        <button type="submit" class="btn primary" disabled={session.submitting} data-autofocus={mode.kind === 'archive' || mode.kind === 'delete' ? '' : undefined}>
          {session.submitting ? 'Saving…' : mode.kind === 'delete' ? 'Delete' : mode.kind === 'archive' ? (mode.category.archivedAt ? 'Restore' : 'Archive') : 'Save'}
        </button>
      </div>
    </form>
  {/if}
</Dialog>

<style>
  .tree li { display: flex; align-items: center; gap: var(--space-2); padding: var(--space-1) var(--space-3); min-height: 40px; border-bottom: 1px solid var(--border); }
  .tree li:last-child { border-bottom: 0; }
  .tree .name { flex: 1; }
  .acts { gap: 0; }
</style>
