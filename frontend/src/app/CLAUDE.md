# src/app/

Bootstrap and wiring: the one place that creates the QueryClient and the router and mounts
React. No product UI lives here.

| File                 | Role                                                                       |
| -------------------- | -------------------------------------------------------------------------- |
| `main.tsx`           | Entry point from `index.html`: creates clients, renders `<AppProviders>`   |
| `providers.tsx`      | `QueryClientProvider` → `RouterProvider`                                   |
| `query-client.ts`    | `createQueryClient()` with app-wide query defaults                         |
| `router.ts`          | `createAppRouter({ queryClient, history? })`, router type registration     |
| `error-boundary.tsx` | `RouteError` (default error component) and `NotFound`                      |
| `routeTree.gen.ts`   | **Generated** by the TanStack router plugin from `src/routes/`; never edit |
| `index.css`          | Tailwind import + the generated token theme; no other global styles        |

## May import

- `src/routes/` (only through `routeTree.gen.ts`), `src/shared/ui`, `src/shared/lib`.
- Not `src/features/` directly: features reach the app only through routes.
- Nothing imports `src/app/` (dependency-cruiser: `routes-not-to-app`, `features-not-to-app-or-routes`).

## Conventions

- Factories, not singletons: `createQueryClient()` and `createAppRouter()` so tests build
  their own with a memory history.
- Router-wide behaviour goes in `createAppRouter` options (`defaultErrorComponent`,
  `defaultPreload`), not in individual routes.
- Every error state ends with a way forward (peak-end): `RouteError` retries, `NotFound`
  links to Agents.
- New providers (theme, telemetry) wrap inside `AppProviders`, outermost first.

## Testing

- `app.test.tsx` renders the whole app with `createMemoryHistory` and asserts through roles.
- Coverage counts toward the 80 % frontend floor; `main.tsx` is excluded (side effects only).
- The browser-level shell behaviour is pinned by `e2e/contract/app-shell/` (contract v1).

## Example

```tsx
// Render the real app at a URL in a test.
const queryClient = createQueryClient();
const router = createAppRouter({
  queryClient,
  history: createMemoryHistory({ initialEntries: ['/settings'] }),
});
render(<AppProviders queryClient={queryClient} router={router} />);
await screen.findByRole('heading', { level: 1, name: 'Settings' });
```
