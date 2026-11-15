/**
 * The /demo graph explorer — the site's only client module.
 *
 * The page already ships a finished drawing: eight perspective panels, each a
 * server-rendered `GraphFigure`, one of them visible. This module draws
 * nothing. It upgrades that markup:
 *
 *   * the switcher becomes a tablist with roving tabindex (the anchors stay
 *     links, so the panels are still reachable when the module never runs);
 *   * every node `<g>` in the visible figure becomes a focusable button;
 *   * Enter or Space on a node fills the panel's detail list and moves focus to
 *     its heading; Escape clears the selection and returns focus to the node;
 *   * the selected node and its immediate neighbours get a class that the
 *     component's stylesheet turns into a highlight (transitioned, and not
 *     under `prefers-reduced-motion`).
 *
 * The payload arrives in a `type="application/json"` block the component
 * renders from `site/data/support-model.json`, so the bundler never sees the
 * projection and the module stays generic. Node `<g>` elements are matched to
 * the payload **by index**: the component emits the projection's node order and
 * `GraphFigure` draws that same array. If the counts disagree the two have
 * drifted, so that panel is left as a static figure rather than mislabelled.
 */

interface NodeRecord {
  /** The graph artifact's id, verbatim. */
  id: string;
  kind: string;
  label: string;
  /** The declaration file, relative to the repository's `examples/` directory. */
  provenance: string;
  incoming: number;
  outgoing: number;
}

interface PerspectiveRecord {
  id: string;
  label: string;
  nodes: NodeRecord[];
  /** Edge endpoints as indexes into `nodes`. */
  edges: Array<[number, number]>;
}

interface Panel {
  element: HTMLElement;
  record: PerspectiveRecord;
  /** `GraphFigure`'s `<g class="node">` elements, in the projection's order. */
  nodes: SVGGElement[];
  /** Immediate neighbours of every node index. */
  neighbours: Map<number, number[]>;
  selected: number | null;
  ready: boolean;
  /** The authored empty state, captured once so clearing can restore it. */
  emptyTitle: string;
  emptyHint: string;
}

/** Read the component's data block; `null` means "leave the page static". */
function readPerspectives(root: HTMLElement): PerspectiveRecord[] | null {
  const text = root.querySelector('script[data-graph-explorer-data]')?.textContent;
  if (!text) return null;
  try {
    const parsed: unknown = JSON.parse(text);
    if (typeof parsed !== 'object' || parsed === null) return null;
    const perspectives = (parsed as { perspectives?: unknown }).perspectives;
    return Array.isArray(perspectives) ? (perspectives as PerspectiveRecord[]) : null;
  } catch {
    return null;
  }
}

function buildNeighbours(record: PerspectiveRecord): Map<number, number[]> {
  const neighbours = new Map<number, number[]>();
  const link = (from: number, to: number): void => {
    const list = neighbours.get(from);
    if (!list) {
      neighbours.set(from, [to]);
      return;
    }
    if (!list.includes(to)) list.push(to);
  };
  for (const [from, to] of record.edges) {
    link(from, to);
    link(to, from);
  }
  return neighbours;
}

function detailOf(panel: Panel): HTMLElement | null {
  return panel.element.querySelector<HTMLElement>('[data-graph-detail]');
}

function factOf(panel: Panel, key: string): HTMLElement | null {
  return detailOf(panel)?.querySelector<HTMLElement>(`[data-graph-fact="${key}"]`) ?? null;
}

/** The human kind word `GraphFigure` already prints under the node's label. */
function kindText(group: Element | undefined, node: NodeRecord): string {
  const printed = group?.querySelector('.node-kind')?.textContent?.trim();
  return printed && printed.length > 0 ? printed : node.kind;
}

/**
 * Make one panel's nodes focusable and map them to the payload. Returns false
 * when the figure and the projection disagree; the caller then leaves the
 * panel alone (a static figure, which is still correct).
 */
function prepare(panel: Panel): boolean {
  if (panel.ready) return true;
  const svg = panel.element.querySelector('svg');
  const groups = svg ? [...svg.querySelectorAll<SVGGElement>('g.node')] : [];
  if (!svg || groups.length !== panel.record.nodes.length) return false;

  // The figure is a group now, not a single image: its children are controls.
  svg.setAttribute('role', 'group');
  groups.forEach((group, index) => {
    const node = panel.record.nodes[index];
    if (!node) return;
    group.setAttribute('tabindex', '0');
    group.setAttribute('role', 'button');
    group.setAttribute('aria-label', `${node.label} ${kindText(groups[index], node)}`);
    group.dataset.nodeIndex = String(index);
  });

  panel.nodes = groups;
  panel.neighbours = buildNeighbours(panel.record);
  panel.ready = true;
  return true;
}

function showDetail(panel: Panel, index: number): void {
  const node = panel.record.nodes[index];
  const detail = detailOf(panel);
  if (!node || !detail) return;

  const title = detail.querySelector<HTMLElement>('[data-graph-detail-title]');
  const hint = detail.querySelector<HTMLElement>('[data-graph-detail-hint]');
  const facts = detail.querySelector<HTMLElement>('[data-graph-detail-facts]');
  if (title) title.textContent = node.label;
  if (hint) hint.textContent = 'Press Escape to clear the selection.';
  if (facts) facts.hidden = false;

  const set = (key: string, value: string): void => {
    const element = factOf(panel, key);
    if (element) element.textContent = value;
  };
  set('kind', kindText(panel.nodes[index], node));
  set('provenance', node.provenance);
  set('connections', `${node.incoming} in · ${node.outgoing} out`);
  set('id', node.id);
}

function hideDetail(panel: Panel): void {
  const detail = detailOf(panel);
  if (!detail) return;
  const title = detail.querySelector<HTMLElement>('[data-graph-detail-title]');
  const hint = detail.querySelector<HTMLElement>('[data-graph-detail-hint]');
  const facts = detail.querySelector<HTMLElement>('[data-graph-detail-facts]');
  if (title) title.textContent = panel.emptyTitle;
  if (hint) hint.textContent = panel.emptyHint;
  if (facts) facts.hidden = true;
}

function clearMarkers(panel: Panel): void {
  for (const group of panel.nodes) group.classList.remove('is-selected', 'is-neighbour');
}

function select(panel: Panel, index: number, toDetail: boolean): void {
  if (!prepare(panel)) return;
  const node = panel.record.nodes[index];
  if (!node) return;

  clearMarkers(panel);
  panel.selected = index;
  panel.nodes[index]?.classList.add('is-selected');
  for (const neighbour of panel.neighbours.get(index) ?? []) {
    panel.nodes[neighbour]?.classList.add('is-neighbour');
  }
  showDetail(panel, index);

  // Keyboard selections hand the reading order to the detail: the heading is
  // focusable and the region is a polite live region, so the facts are one
  // ArrowDown away. A pointer selection leaves focus where the visitor put it.
  if (toDetail) detailOf(panel)?.querySelector<HTMLElement>('[data-graph-detail-title]')?.focus();
}

function clear(panel: Panel, toNode: boolean): void {
  if (panel.selected === null) return;
  const index = panel.selected;
  clearMarkers(panel);
  panel.selected = null;
  hideDetail(panel);
  if (toNode) panel.nodes[index]?.focus();
}

function enhance(root: HTMLElement): void {
  const records = readPerspectives(root);
  const tablist = root.querySelector<HTMLElement>('[role="tablist"]');
  if (!records || !tablist) return;

  const tabs = [...root.querySelectorAll<HTMLAnchorElement>('[data-graph-tab]')];
  const panels = new Map<string, Panel>();

  for (const element of root.querySelectorAll<HTMLElement>('[data-graph-panel]')) {
    const id = element.dataset.graphPanel;
    const record = id ? records.find((candidate) => candidate.id === id) : undefined;
    if (!id || !record) continue;
    const detail = element.querySelector<HTMLElement>('[data-graph-detail]');
    panels.set(id, {
      element,
      record,
      nodes: [],
      neighbours: new Map(),
      selected: null,
      ready: false,
      emptyTitle:
        detail?.querySelector<HTMLElement>('[data-graph-detail-title]')?.textContent ?? 'Nothing selected',
      emptyHint: detail?.querySelector<HTMLElement>('[data-graph-detail-hint]')?.textContent ?? '',
    });
  }

  if (tabs.length === 0 || panels.size === 0) return;

  // From here the stylesheet knows JavaScript is live: the `:target` reveal
  // rule stops applying and the panels are toggled by the module instead.
  root.dataset.enhanced = '';

  const activate = (id: string, focusTab: boolean): void => {
    for (const tab of tabs) {
      const selected = tab.dataset.graphTab === id;
      tab.setAttribute('aria-selected', String(selected));
      tab.tabIndex = selected ? 0 : -1;
      if (selected && focusTab) tab.focus();
    }
    for (const [panelId, panel] of panels) {
      const active = panelId === id;
      // `data-collapsed`, not `hidden`: the component owns the hide rule so the
      // no-JS `:target` reveal is not fighting Tailwind's important preflight.
      panel.element.toggleAttribute('data-collapsed', !active);
      if (active) prepare(panel);
    }
  };

  const tabOf = (target: EventTarget | null): HTMLAnchorElement | null =>
    target instanceof Element ? target.closest<HTMLAnchorElement>('[data-graph-tab]') : null;

  tablist.addEventListener('keydown', (event) => {
    const tab = tabOf(event.target);
    if (!tab) return;
    const index = tabs.indexOf(tab);
    if (index < 0) return;

    if (event.key === 'Enter' || event.key === ' ') {
      // An unenhanced anchor would jump to the hash; here the panel switches.
      event.preventDefault();
      const id = tab.dataset.graphTab;
      if (id) activate(id, true);
      return;
    }

    const last = tabs.length - 1;
    let next: number;
    if (event.key === 'ArrowRight') next = index === last ? 0 : index + 1;
    else if (event.key === 'ArrowLeft') next = index === 0 ? last : index - 1;
    else if (event.key === 'Home') next = 0;
    else if (event.key === 'End') next = last;
    else return;

    const id = tabs[next]?.dataset.graphTab;
    if (!id) return;
    event.preventDefault();
    activate(id, true);
  });

  tablist.addEventListener('click', (event) => {
    const id = tabOf(event.target)?.dataset.graphTab;
    if (!id) return;
    event.preventDefault();
    activate(id, false);
  });

  /** A click or key on a prepared node, resolved to its panel and index. */
  const hit = (target: EventTarget | null): { panel: Panel; index: number } | null => {
    const group = target instanceof Element ? target.closest('g.node') : null;
    const id = group?.closest<HTMLElement>('[data-graph-panel]')?.dataset.graphPanel;
    const panel = id ? panels.get(id) : undefined;
    if (!group || !panel || !panel.ready) return null;
    const index = Number((group as SVGGElement).dataset.nodeIndex);
    return Number.isInteger(index) && panel.nodes[index] === group ? { panel, index } : null;
  };

  root.addEventListener('click', (event) => {
    const target = hit(event.target);
    if (!target) return;
    event.preventDefault();
    select(target.panel, target.index, false);
  });

  root.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') {
      for (const panel of panels.values()) {
        // Only the visible layer can have a selection to clear; a panel the
        // visitor switched away from keeps its mark for the way back.
        if (panel.element.hasAttribute('data-collapsed') || panel.selected === null) continue;
        event.preventDefault();
        clear(panel, true);
        return;
      }
      return;
    }
    if (event.key !== 'Enter' && event.key !== ' ') return;
    const target = hit(event.target);
    if (!target) return;
    event.preventDefault();
    select(target.panel, target.index, true);
  });

  // Reflect the server-rendered selection: one tabbable tab, one visible panel.
  // A `#graph-panel-…` in the URL wins, so a shared link to a layer opens that
  // layer with JavaScript on exactly as it does with it off (`:target`).
  const linked = location.hash.match(/^#graph-panel-(.+)$/)?.[1];
  const current = tabs.find((tab) => tab.getAttribute('aria-selected') === 'true') ?? tabs[0];
  const currentId = linked && panels.has(linked) ? linked : current?.dataset.graphTab;
  if (currentId) activate(currentId, false);
}

for (const root of document.querySelectorAll<HTMLElement>('[data-graph-explorer]')) {
  enhance(root);
}
