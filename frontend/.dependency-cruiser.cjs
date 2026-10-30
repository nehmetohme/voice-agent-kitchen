// Frontend layer rules: the single source of truth for import boundaries.
// See src/features/CLAUDE.md and docs/research/03-frontend.md §3.
//
// Allowed direction: app → routes → features/*/index.ts; inside a feature, views → state → api.
// Every layer may use shared/ui and shared/lib. Only features/*/api touches the generated client.
//
// TypeScript 7 has no compiler API, so dependency-cruiser parses .ts/.tsx with swc.

/** @type {import('dependency-cruiser').IConfiguration} */
module.exports = {
  forbidden: [
    {
      name: 'no-circular',
      severity: 'error',
      comment: 'Cycles make layers meaningless. Move the shared part down a layer.',
      from: {},
      to: { circular: true },
    },
    {
      name: 'no-cross-feature',
      severity: 'error',
      comment:
        'A feature never imports another feature. Compose them in routes/ or move code to shared/.',
      from: { path: '^src/features/([^/]+)/' },
      to: { path: '^src/features/([^/]+)/', pathNot: '^src/features/$1/' },
    },
    {
      name: 'feature-public-api-only',
      severity: 'error',
      comment: 'Outside a feature, import it only through its index.ts.',
      from: { pathNot: '^src/features/' },
      to: { path: '^src/features/[^/]+/', pathNot: '^src/features/[^/]+/index\\.ts$' },
    },
    {
      name: 'views-not-to-api',
      severity: 'error',
      comment: 'Views never fetch. They get data from state/ hooks.',
      from: { path: '^src/features/[^/]+/views/' },
      to: { path: ['^src/features/[^/]+/api/', '^src/shared/api/'] },
    },
    {
      name: 'state-not-to-views',
      severity: 'error',
      comment: 'state/ holds query options, machines and stores; it renders nothing.',
      from: { path: '^src/features/[^/]+/state/' },
      to: { path: '^src/features/[^/]+/views/' },
    },
    {
      name: 'api-not-to-state-or-views',
      severity: 'error',
      comment: 'api/ wraps the generated client and maps DTOs; it knows nothing above it.',
      from: { path: '^src/features/[^/]+/api/' },
      to: { path: '^src/features/[^/]+/(state|views)/' },
    },
    {
      name: 'generated-client-only-from-feature-api',
      severity: 'error',
      comment: 'Only features/*/api (and shared/api itself) may import the generated client.',
      from: { pathNot: ['^src/features/[^/]+/api/', '^src/shared/api/'] },
      to: { path: '^src/shared/api/generated/' },
    },
    {
      name: 'shared-not-to-upper-layers',
      severity: 'error',
      comment: 'shared/ is the bottom layer: it never imports app, routes or features.',
      from: { path: '^src/shared/' },
      to: { path: '^src/(app|routes|features)/' },
    },
    {
      name: 'features-not-to-app-or-routes',
      severity: 'error',
      comment: 'Features are composed by routes, never the other way round.',
      from: { path: '^src/features/' },
      to: { path: '^src/(app|routes)/' },
    },
    {
      name: 'routes-not-to-app',
      severity: 'error',
      comment: 'app/ wires the router from routes/; routes never import app/.',
      from: { path: '^src/routes/' },
      to: { path: '^src/app/' },
    },
    {
      name: 'base-ui-only-in-shared-ui',
      severity: 'error',
      comment:
        'Only shared/ui wraps Base UI primitives. Everything else uses shared/ui components.',
      from: { pathNot: '^src/shared/ui/' },
      to: { path: '(^|/)node_modules/@base-ui/react/' },
    },
    {
      name: 'no-test-code-in-production',
      severity: 'error',
      comment: 'Production code never imports tests or test helpers.',
      from: { pathNot: ['\\.(test|stories)\\.tsx?$', '^src/test/'] },
      to: { path: ['\\.(test|stories)\\.tsx?$', '^src/test/'] },
    },
    {
      name: 'no-dev-dependencies-in-production',
      severity: 'error',
      comment: 'Production code only imports runtime dependencies.',
      from: { path: '^src/', pathNot: ['\\.(test|stories)\\.tsx?$', '^src/test/'] },
      to: { dependencyTypes: ['npm-dev'], dependencyTypesNot: ['type-only'] },
    },
    {
      name: 'no-unresolvable',
      severity: 'error',
      comment: 'Every import must resolve.',
      from: {},
      to: { couldNotResolve: true },
    },
  ],
  options: {
    parser: 'swc',
    doNotFollow: { path: 'node_modules' },
    exclude: { path: '\\.css$' },
    moduleSystems: ['es6'],
    combinedDependencies: false,
    enhancedResolveOptions: {
      exportsFields: ['exports'],
      conditionNames: ['import', 'browser', 'default', 'types'],
      extensions: ['.ts', '.tsx', '.js', '.mjs', '.json', '.d.ts'],
      mainFields: ['module', 'main', 'types'],
    },
    reporterOptions: {
      text: { highlightFocused: true },
    },
  },
};
