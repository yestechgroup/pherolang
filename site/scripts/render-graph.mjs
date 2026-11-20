#!/usr/bin/env node
/**
 * Derive `site/data/support-model.json` — the demo projection of the semantic
 * graph — from the committed `site/data/graph.json`.
 *
 * WHY THIS FILE EXISTS
 * --------------------
 * The graph artifact is one flat node/edge table. A visitor needs
 * *perspectives*: the domain model, the decisions, the events, the workflows,
 * the policy, the interaction flow, the deployment, the case, the
 * requirements — the same application seen through each surface. And
 * the page must paint before JavaScript runs, so the layout is **baked here**,
 * not computed in the browser. GraphFigure.astro renders these numbers
 * directly into SVG; the interactive island (lane 6) reuses the same nodes and
 * edges and only adds focus, filtering and the detail panel.
 *
 * LAWS
 * ----
 * 1. **Derived, never invented.** Every node, edge, id and provenance string
 *    below is copied from `graph.json` verbatim. The only derived values are
 *    presentation: the short label, the cluster key, the sentence, the
 *    geometry.
 * 2. **Closed under its edges.** A perspective keeps an edge only when both
 *    endpoints are in that perspective. The script asserts the property on the
 *    model it is about to write and exits 1 otherwise: a dangling endpoint is
 *    a bug here, never a silent drop in the page.
 * 3. **Deterministic.** Every array is sorted, the layout is a pure function of
 *    the sorted node list, and nothing reads the clock, the filesystem order or
 *    a locale. `node scripts/render-graph.mjs` twice produces identical bytes.
 * 4. **Every node is classified.** A node kind that no perspective claims (and
 *    that is not in EXCLUDED_KINDS) is a hard error: when the graph grows a new
 *    kind the site must decide where it belongs instead of quietly losing it.
 *
 * OUTPUT SHAPE (the contract lane 6 codes against; see site/data/CONTRACT.md)
 * --------------------------------------------------------------------------
 *   { formatVersion, application, meta, perspectives[ { id, label, surface,
 *     viewport, groups[], nodes[], edges[] } ] }
 *
 * Additive fields beyond site/data/CONTRACT.md:
 *   `groups[]`   the cluster headers the figure draws (key, label, box, count);
 *   `viewport`   the baked SVG size, so no template has to run layout;
 *   `meta`       provenance + per-perspective counts for the architecture page.
 * A node keeps the contract's exact shape:
 *   { id, kind, label, group, x, y, w, h, detail, provenance }.
 *
 * Usage: node scripts/render-graph.mjs   (from `site/`; also chained by
 *        scripts/sync-model.mjs after `pherolang graph build`)
 */

import { readFile, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const SITE = dirname(dirname(fileURLToPath(import.meta.url)));
const GRAPH_PATH = join(SITE, 'data', 'graph.json');
const OUT_PATH = join(SITE, 'data', 'support-model.json');

/** Display paths as written in the artifact and in the contract. */
const GRAPH_DISPLAY = 'site/data/graph.json';
const OUT_DISPLAY = 'site/data/support-model.json';

// ---------------------------------------------------------------------------
// The perspective table — the one place a node kind is assigned to a view.
//
// Kind order matters twice: it is the reading order inside a cluster (a class
// before its features, a service before its operations) and it is the order
// the legend lists. Every kind the support application declares must appear
// here (law 4); `file` is deliberately excluded — it names a host fact, not a
// declaration, and its `imports` edges connect files, not the model.
// ---------------------------------------------------------------------------
const PERSPECTIVES = [
  {
    id: 'domain',
    label: 'Domain',
    kinds: ['class', 'enum', 'datatype', 'feature', 'operation'],
  },
  {
    id: 'decisions',
    label: 'Decisions',
    kinds: [
      'decisionModel',
      'decision',
      'decisionService',
      'bkm',
      'knowledgeSource',
      'inputData',
    ],
  },
  {
    id: 'events',
    label: 'Events',
    kinds: ['event', 'channel', 'subscription'],
  },
  {
    id: 'workflows',
    label: 'Workflows',
    kinds: [
      'service',
      'serviceOperation',
      'repository',
      'repositoryOperation',
      'module',
      'designApplication',
      'search',
      'dto',
      'error',
    ],
  },
  {
    id: 'authorisation',
    label: 'Authorisation',
    kinds: ['actor', 'capability'],
  },
  {
    id: 'experience',
    label: 'Experience',
    kinds: ['flow', 'view', 'component', 'action', 'command'],
  },
  {
    id: 'deployment',
    label: 'Deployment',
    kinds: ['deployment', 'deployApplication', 'deployComponent', 'deployProfile'],
  },
  {
    id: 'cases',
    label: 'Cases',
    kinds: ['case', 'planItem', 'caseFileItem', 'caseRole', 'useCase'],
  },
  {
    id: 'intent',
    label: 'Requirements',
    kinds: ['intent'],
  },
];

/**
 * Kinds with no perspective on purpose. `file` is the graph's provenance
 * anchor (`file:<path>#<path>`); it would be a box in every view and carry no
 * model information, and the node's own `provenance` field already names the
 * declaration file. This list is asserted against the artifact, so it cannot
 * rot silently.
 */
const EXCLUDED_KINDS = ['file'];

/** Human kind phrase, used to build the node's one-sentence `detail`. */
const KIND_NOUN = {
  class: 'a class',
  enum: 'an enum',
  datatype: 'a datatype',
  feature: 'a feature',
  operation: 'an operation',
  decisionModel: 'a decision model',
  decision: 'a decision',
  decisionService: 'a decision service',
  bkm: 'a business knowledge model',
  knowledgeSource: 'a knowledge source',
  inputData: 'an input data node',
  event: 'an event',
  channel: 'a channel',
  subscription: 'a subscription',
  service: 'a service',
  serviceOperation: 'a service operation',
  repository: 'a repository',
  repositoryOperation: 'a repository operation',
  module: 'a module',
  designApplication: 'a design application',
  search: 'a search',
  dto: 'a data-transfer object',
  error: 'a declared error',
  actor: 'an actor',
  capability: 'a capability',
  intent: 'a requirement',
  flow: 'an interaction flow',
  view: 'a view',
  component: 'a component',
  action: 'an action',
  command: 'a command',
  deployment: 'a deployment',
  deployApplication: 'a deployment application',
  deployComponent: 'a deployment component',
  deployProfile: 'a deployment profile',
  case: 'a case',
  planItem: 'a plan item',
  caseFileItem: 'a case-file item',
  caseRole: 'a case role',
  useCase: 'a use case',
};

// ---------------------------------------------------------------------------
// Baked geometry. One column per `group`, at most two node sub-columns inside
// a column, nodes laid out row-major. Sizes are SVG user units (px at 1×); the
// figure never scales text below 11px — it scrolls instead.
// ---------------------------------------------------------------------------
const LAYOUT = {
  margin: 20,
  columnGap: 40,
  subColumnGap: 12,
  rowGap: 12,
  nodeWidth: 176,
  nodeHeight: 46,
  groupHeader: 34,
  maxSubColumns: 2,
};

function fail(message) {
  console.error(`render-graph: ${message}`);
  process.exit(1);
}

/** The last segment of a qualified name — the short human label. */
function shortLabel(name) {
  const parts = name.split('.').filter(Boolean);
  return parts.length ? parts[parts.length - 1] : name;
}

/** `support/support.mox` → `support`; used only when a name has no qualifier. */
function fileStem(file) {
  const base = file.slice(file.lastIndexOf('/') + 1);
  return base.slice(0, base.lastIndexOf('.')) || base;
}

/**
 * Kinds that declare a whole model inside one file: the decision model, the
 * design application, the deployment application, the case and the interaction
 * flow. They anchor the cluster for their file's top-level declarations.
 */
const ROOT_KINDS = new Set(['decisionModel', 'designApplication', 'deployApplication', 'case', 'flow']);

/**
 * The cluster key: the declaration that owns the node — the qualified name
 * minus its last segment. `nz.example.support.Ticket.refundCents` groups under
 * `nz.example.support.Ticket`, `Support.RaiseRefund` under `Support`,
 * `RefundDesk.ApproveRefund` under `RefundDesk`.
 *
 * A **top-level** declaration has no owner in its name, so it groups under the
 * model root of its file: the file's root-kind node (`decisionModel`, `case`,
 * `designApplication`, `flow`, …) when it declares one, else the file stem.
 * That keeps a case with its plan items instead of splitting the case into a
 * cluster named after the file.
 */
function groupKey(node, rootByFile) {
  const parts = node.name.split('.').filter(Boolean);
  if (parts.length > 1) return parts.slice(0, -1).join('.');
  return rootByFile.get(node.file) ?? fileStem(node.file);
}

/** The surface whose nodes dominate a perspective, from the declaring files. */
function dominantSurface(nodes) {
  const counts = new Map();
  for (const node of nodes) {
    const dot = node.file.lastIndexOf('.');
    const extension = dot >= 0 ? node.file.slice(dot + 1) : 'unknown';
    counts.set(extension, (counts.get(extension) ?? 0) + 1);
  }
  // Byte-order tie-break: no locale, no insertion-order dependence.
  return [...counts.entries()].sort(([a, an], [b, bn]) => bn - an || (a < b ? -1 : a > b ? 1 : 0))[0]?.[0] ?? 'unknown';
}

/** One sentence, built only from the node's kind, file and in-view degree. */
function detailFor(node, degree) {
  const noun = KIND_NOUN[node.kind] ?? `a ${node.kind}`;
  const connections =
    degree === 1 ? 'one connection shown in this view' : `${degree} connections shown in this view`;
  return `${shortLabel(node.name)} is ${noun} declared in ${node.file}, with ${connections}.`;
}

/**
 * Lay one perspective out. Pure: the same sorted node list always yields the
 * same coordinates.
 */
function layout(nodes) {
  const {
    margin,
    columnGap,
    subColumnGap,
    rowGap,
    nodeWidth,
    nodeHeight,
    groupHeader,
    maxSubColumns,
  } = LAYOUT;

  const byGroup = new Map();
  for (const node of nodes) {
    const key = node.group;
    if (!byGroup.has(key)) byGroup.set(key, []);
    byGroup.get(key).push(node);
  }

  const keys = [...byGroup.keys()].sort();
  const placed = [];
  const placeGroups = [];
  let x = margin;
  let tallest = 0;

  for (const key of keys) {
    const members = byGroup.get(key);
    const subColumns = Math.min(maxSubColumns, Math.max(1, members.length));
    const rows = Math.ceil(members.length / subColumns);
    const width = subColumns * nodeWidth + (subColumns - 1) * subColumnGap;
    const height = groupHeader + rows * nodeHeight + (rows - 1) * rowGap;

    members.forEach((node, index) => {
      const column = index % subColumns;
      const row = Math.floor(index / subColumns);
      placed.push({
        ...node,
        group: key,
        x: x + column * (nodeWidth + subColumnGap),
        y: margin + groupHeader + row * (nodeHeight + rowGap),
        w: nodeWidth,
        h: nodeHeight,
      });
    });

    placeGroups.push({ key, label: key, x, y: margin, w: width, h: height, count: members.length });
    tallest = Math.max(tallest, height);
    x += width + columnGap;
  }

  const viewport = {
    width: keys.length ? x - columnGap + margin : margin * 2,
    height: margin * 2 + tallest,
  };

  return { nodes: placed, groups: placeGroups, viewport };
}

/** Assert the baked geometry is sane: inside the viewport, no overlaps. */
function assertGeometry(perspective) {
  const { id, nodes, groups, viewport } = perspective;
  const overlap = (a, b) => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;

  for (const node of nodes) {
    if (node.x < 0 || node.y < 0 || node.x + node.w > viewport.width || node.y + node.h > viewport.height) {
      fail(`${id}: ${node.id} falls outside the ${viewport.width}x${viewport.height} viewport`);
    }
    const group = groups.find((candidate) => candidate.key === node.group);
    if (!group) fail(`${id}: ${node.id} has no group box`);
    if (node.x < group.x || node.y < group.y + LAYOUT.groupHeader) {
      fail(`${id}: ${node.id} collides with the ${group.key} header`);
    }
    if (node.x + node.w > group.x + group.w || node.y + node.h > group.y + group.h) {
      fail(`${id}: ${node.id} overflows the ${group.key} group box`);
    }
  }

  for (let i = 0; i < nodes.length; i += 1) {
    for (let j = i + 1; j < nodes.length; j += 1) {
      if (overlap(nodes[i], nodes[j])) {
        fail(`${id}: ${nodes[i].id} overlaps ${nodes[j].id}`);
      }
    }
  }
}

/** Assert the model is closed, unique and complete. Exit 1 on any violation. */
function assertModel(model, selected, expected) {
  const claimed = new Map();

  for (const perspective of model.perspectives) {
    const ids = new Set();
    for (const node of perspective.nodes) {
      if (ids.has(node.id)) fail(`${perspective.id}: duplicate node id ${node.id}`);
      ids.add(node.id);

      const previous = claimed.get(node.id);
      if (previous) fail(`${node.id} is claimed by both ${previous} and ${perspective.id}`);
      claimed.set(node.id, perspective.id);

      for (const key of ['x', 'y', 'w', 'h']) {
        if (!Number.isInteger(node[key])) fail(`${node.id}: ${key} is not an integer`);
      }
      if (typeof node.label !== 'string' || !node.label) fail(`${node.id}: empty label`);
      if (node.label.includes('.')) fail(`${node.id}: label ${node.label} is not the short name`);
      if (!node.group) fail(`${node.id}: empty group key`);
    }

    for (const edge of perspective.edges) {
      if (!ids.has(edge.from)) fail(`${perspective.id}: edge from ${edge.from} is outside the view`);
      if (!ids.has(edge.to)) fail(`${perspective.id}: edge to ${edge.to} is outside the view`);
      if (!edge.kind) fail(`${perspective.id}: edge ${edge.from} -> ${edge.to} has no kind`);
    }

    assertGeometry(perspective);
  }

  for (const node of model.perspectives.flatMap((perspective) => perspective.nodes)) {
    const source = selected.get(node.id);
    if (!source) fail(`unknown node in the projection: ${node.id}`);
    if (node.kind !== source.kind) fail(`${node.id}: kind changed to ${node.kind}`);
    if (node.provenance !== source.file) fail(`${node.id}: provenance changed`);
    if (node.id.includes('#') === false) fail(`${node.id}: not a graph node id`);
  }

  if (claimed.size !== expected) {
    fail(`classified ${claimed.size} nodes but expected ${expected} (unclaimed kinds?)`);
  }
}

// ---------------------------------------------------------------------------
// Derive.
// ---------------------------------------------------------------------------
const graph = JSON.parse(await readFile(GRAPH_PATH, 'utf8'));
const nodes = graph.nodes ?? [];
const edges = graph.edges ?? [];
const applications = graph.applications ?? [];

if (!nodes.length) fail(`${GRAPH_DISPLAY} has no nodes — run scripts/sync-model.mjs first`);

/**
 * The demo application. Picked by size (most files), so it stays the full-stack
 * one if the corpus is reordered; the id is whatever the artifact says.
 */
const application = [...applications].sort(
  (a, b) => (b.files?.length ?? 0) - (a.files?.length ?? 0) || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0),
)[0];
if (!application) fail(`${GRAPH_DISPLAY} has no applications`);

const selectedNodes = nodes.filter((node) => node.application === application.id);
if (!selectedNodes.length) fail(`application ${application.id} has no nodes`);

const selectedIds = new Set(selectedNodes.map((node) => node.id));
const selectedEdges = edges.filter((edge) => selectedIds.has(edge.from) && selectedIds.has(edge.to));

/**
 * One model root per file, taken from its root-kind declaration. Sorted by id
 * first so the choice never depends on artifact order.
 */
const rootByFile = new Map();
for (const node of [...selectedNodes].sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0))) {
  if (ROOT_KINDS.has(node.kind) && !rootByFile.has(node.file)) rootByFile.set(node.file, node.name);
}

// Which kinds exist, and are they all classified?
const perspectiveByKind = new Map();
for (const perspective of PERSPECTIVES) {
  for (const kind of perspective.kinds) {
    if (perspectiveByKind.has(kind)) fail(`kind ${kind} is claimed by two perspectives`);
    perspectiveByKind.set(kind, perspective.id);
  }
}
for (const kind of EXCLUDED_KINDS) {
  if (perspectiveByKind.has(kind)) fail(`excluded kind ${kind} is also claimed by a perspective`);
}

const presentKinds = [...new Set(selectedNodes.map((node) => node.kind))].sort();
for (const kind of presentKinds) {
  if (perspectiveByKind.has(kind) || EXCLUDED_KINDS.includes(kind)) continue;
  fail(
    `node kind '${kind}' belongs to no perspective.\n` +
      '  Add it to PERSPECTIVES (and say which view it belongs to) — the site must\n' +
      '  not silently drop a declaration kind the graph has started emitting.',
  );
}

const classifiedKinds = new Set(
  presentKinds.filter((kind) => perspectiveByKind.has(kind)),
);
const expectedClassified = selectedNodes.filter((node) => classifiedKinds.has(node.kind)).length;

// Edges between two nodes of the same perspective only: an edge into another
// view says nothing inside this one, and drawing it would need a phantom node.
const perspectives = PERSPECTIVES.map((perspective) => {
  const kindRank = new Map(perspective.kinds.map((kind, index) => [kind, index]));
  const members = selectedNodes
    .filter((node) => kindRank.has(node.kind))
    .map((node) => ({
      id: node.id,
      kind: node.kind,
      name: node.name,
      file: node.file,
    }))
    .sort(
      (a, b) =>
        kindRank.get(a.kind) - kindRank.get(b.kind) ||
        (shortLabel(a.name) < shortLabel(b.name) ? -1 : shortLabel(a.name) > shortLabel(b.name) ? 1 : 0) ||
        (a.id < b.id ? -1 : a.id > b.id ? 1 : 0),
    );

  const memberIds = new Set(members.map((member) => member.id));
  const viewEdges = selectedEdges
    .filter((edge) => memberIds.has(edge.from) && memberIds.has(edge.to))
    .map((edge) => ({ from: edge.from, to: edge.to, kind: edge.kind }))
    .sort(
      (a, b) =>
        (a.from < b.from ? -1 : a.from > b.from ? 1 : 0) ||
        (a.to < b.to ? -1 : a.to > b.to ? 1 : 0) ||
        (a.kind < b.kind ? -1 : a.kind > b.kind ? 1 : 0),
    );

  const viewDegree = new Map();
  for (const edge of viewEdges) {
    viewDegree.set(edge.from, (viewDegree.get(edge.from) ?? 0) + 1);
    viewDegree.set(edge.to, (viewDegree.get(edge.to) ?? 0) + 1);
  }

  const projected = members.map((member) => ({
    id: member.id,
    kind: member.kind,
    label: shortLabel(member.name),
    group: groupKey(member, rootByFile),
    x: 0,
    y: 0,
    w: 0,
    h: 0,
    detail: detailFor({ kind: member.kind, name: member.name, file: member.file }, viewDegree.get(member.id) ?? 0),
    provenance: member.file,
  }));

  const laid = layout(projected);

  return {
    id: perspective.id,
    label: perspective.label,
    surface: dominantSurface(members.map((member) => ({ file: member.file }))),
    viewport: laid.viewport,
    groups: laid.groups,
    nodes: laid.nodes,
    edges: viewEdges,
  };
});

const meta = {
  application: application.id,
  files: [...(application.files ?? [])].sort(),
  source: GRAPH_DISPLAY,
  generator: 'site/scripts/render-graph.mjs',
  graph: { nodes: nodes.length, edges: edges.length, applications: applications.length },
  excludedKinds: [...EXCLUDED_KINDS],
  excludedNodes: selectedNodes.length - expectedClassified,
  layout: LAYOUT,
  perspectives: Object.fromEntries(
    perspectives.map((perspective) => [
      perspective.id,
      {
        nodes: perspective.nodes.length,
        edges: perspective.edges.length,
        groups: perspective.groups.length,
      },
    ]),
  ),
};

const model = {
  formatVersion: 1,
  application: application.id,
  meta,
  perspectives,
};

assertModel(model, new Map(selectedNodes.map((node) => [node.id, node])), expectedClassified);

// A perspective that claims a node kind but holds no node is still emitted (the
// eight ids are the contract); it must simply be empty rather than wrong.
await writeFile(OUT_PATH, `${JSON.stringify(model, null, 2)}\n`);

// ---------------------------------------------------------------------------
// Report — the numbers in the summary are what the site will draw.
// ---------------------------------------------------------------------------
const totalEdges = perspectives.reduce((sum, perspective) => sum + perspective.edges.length, 0);
console.log(`render-graph: ${OUT_DISPLAY}`);
console.log(`  application ${application.id} (${application.files?.length ?? 0} files)`);
console.log(
  `  ${perspectives.reduce((sum, perspective) => sum + perspective.nodes.length, 0)} nodes / ` +
    `${totalEdges} intra-perspective edges of ${selectedEdges.length} application edges ` +
    `(${selectedNodes.length - expectedClassified} ${EXCLUDED_KINDS.join('/')} nodes excluded)`,
);
for (const perspective of perspectives) {
  console.log(
    `  ${perspective.id.padEnd(14)} ${String(perspective.nodes.length).padStart(3)} nodes ` +
      `${String(perspective.edges.length).padStart(3)} edges ` +
      `${String(perspective.groups.length).padStart(2)} groups ` +
      `${perspective.viewport.width}x${perspective.viewport.height}`,
  );
}
