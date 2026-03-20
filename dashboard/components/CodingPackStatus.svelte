<script lang="ts">
  import { onMount } from 'svelte';
  import { getClientOrNull } from '$lib/api/client';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import CheckCircle2Icon from '@lucide/svelte/icons/check-circle-2';
  import XCircleIcon from '@lucide/svelte/icons/x-circle';
  import AlertTriangleIcon from '@lucide/svelte/icons/alert-triangle';
  import RefreshCwIcon from '@lucide/svelte/icons/refresh-cw';
  import PackageIcon from '@lucide/svelte/icons/package';
  import Loader2Icon from '@lucide/svelte/icons/loader-2';

  interface PackStatus {
    valid: boolean;
    plugins_ok: number;
    workflows_found: number;
    issues: string[];
    plugins?: Array<{ name: string; size_bytes: number; executable: boolean }>;
    workflows?: string[];
  }

  let status = $state<PackStatus | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  async function fetchStatus(): Promise<void> {
    const client = getClientOrNull();
    if (!client) {
      loading = false;
      return;
    }

    try {
      loading = true;
      error = null;

      // Try to fetch pack status via plugin action API
      const result = await client.pluginRequest<PackStatus>(
        'plugin-coding-pack',
        'status'
      );
      status = result;
    } catch (err) {
      // If the pack status endpoint isn't available, try listing plugins instead
      try {
        const plugins = await client.listPlugins();
        const packPlugin = plugins.find(
          (p) => p.name === 'plugin-coding-pack' || p.name === 'coding-pack'
        );

        if (packPlugin) {
          const workflows = await client.listWorkflows();
          status = {
            valid: true,
            plugins_ok: plugins.length,
            workflows_found: workflows.length,
            issues: [],
            workflows: workflows.map((w) => w.id)
          };
        } else {
          error = 'Coding pack not detected';
        }
      } catch {
        error = err instanceof Error ? err.message : 'Failed to fetch status';
      }
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    fetchStatus();
  });
</script>

<div class="pack-status">
  <div class="status-header">
    <h3 class="section-title">
      <PackageIcon size={16} />
      Coding Pack
    </h3>
    <Button variant="ghost" size="sm" onclick={fetchStatus} disabled={loading}>
      {#if loading}
        <Loader2Icon size={14} class="animate-spin" />
      {:else}
        <RefreshCwIcon size={14} />
      {/if}
    </Button>
  </div>

  {#if loading && !status}
    <div class="status-loading">
      <Loader2Icon size={20} class="animate-spin" />
      <span>Checking pack status...</span>
    </div>
  {:else if error}
    <div class="status-row error">
      <AlertTriangleIcon size={16} />
      <span>{error}</span>
    </div>
  {:else if status}
    <div class="status-grid">
      <div class="status-item" class:ok={status.valid} class:fail={!status.valid}>
        {#if status.valid}
          <CheckCircle2Icon size={16} />
        {:else}
          <XCircleIcon size={16} />
        {/if}
        <span class="status-label">Pack Valid</span>
      </div>

      <div class="status-item ok">
        <span class="status-value">{status.plugins_ok}</span>
        <span class="status-label">Plugins</span>
      </div>

      <div class="status-item ok">
        <span class="status-value">{status.workflows_found}</span>
        <span class="status-label">Workflows</span>
      </div>
    </div>

    {#if status.issues && status.issues.length > 0}
      <div class="issues-list">
        {#each status.issues as issue}
          <div class="issue-item">
            <AlertTriangleIcon size={12} />
            <span>{issue}</span>
          </div>
        {/each}
      </div>
    {/if}

    {#if status.plugins && status.plugins.length > 0}
      <div class="plugin-list">
        {#each status.plugins as plugin}
          <div class="plugin-row">
            <span class="plugin-name">{plugin.name}</span>
            <Badge variant={plugin.executable ? 'secondary' : 'destructive'} class="text-xs">
              {plugin.executable ? 'OK' : 'Missing'}
            </Badge>
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</div>

<style>
  .pack-status {
    background: var(--pulse-surface);
    border: 1px solid var(--pulse-border);
    border-radius: var(--radius-lg, 0.75rem);
    padding: 1.25rem;
  }

  .status-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.75rem;
  }

  .section-title {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--pulse-text);
    margin: 0;
  }

  .section-title :global(svg) {
    color: var(--pulse-success);
  }

  .status-loading {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0;
    font-size: 0.8125rem;
    color: var(--pulse-text-secondary);
  }

  .status-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 0.5rem;
  }

  .status-item {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
    padding: 0.625rem;
    background: var(--pulse-bg);
    border-radius: var(--radius-md, 0.5rem);
    border: 1px solid var(--pulse-border);
  }

  .status-item.ok :global(svg) { color: var(--pulse-success); }
  .status-item.fail :global(svg) { color: var(--pulse-error); }

  .status-value {
    font-size: 1.25rem;
    font-weight: 700;
    color: var(--pulse-text);
    font-variant-numeric: tabular-nums;
    line-height: 1;
  }

  .status-label {
    font-size: 0.6875rem;
    font-weight: 500;
    color: var(--pulse-text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .status-row.error {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0.75rem;
    background: oklch(0.25 0.08 25 / 0.3);
    border: 1px solid oklch(0.35 0.08 25 / 0.3);
    border-radius: var(--radius-md, 0.5rem);
    font-size: 0.8125rem;
    color: oklch(0.75 0.1 25);
  }

  .issues-list {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    margin-top: 0.75rem;
  }

  .issue-item {
    display: flex;
    align-items: flex-start;
    gap: 0.375rem;
    font-size: 0.75rem;
    color: var(--pulse-warning);
    line-height: 1.4;
  }

  .issue-item :global(svg) {
    flex-shrink: 0;
    margin-top: 1px;
  }

  .plugin-list {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    margin-top: 0.75rem;
    padding-top: 0.75rem;
    border-top: 1px solid var(--pulse-border);
  }

  .plugin-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.25rem 0;
  }

  .plugin-name {
    font-size: 0.75rem;
    font-family: 'SF Mono', Monaco, Consolas, monospace;
    color: var(--pulse-text);
  }
</style>
