<script lang="ts">
  import { onMount } from 'svelte';
  import { codingPack, WORKFLOWS, AGENTS, type WorkflowMeta } from '../../stores/codingPack.svelte';
  import ExecuteWorkflowDialog from '../../components/ExecuteWorkflowDialog.svelte';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import PackageIcon from '@lucide/svelte/icons/package';
  import CheckCircle2Icon from '@lucide/svelte/icons/check-circle-2';
  import XCircleIcon from '@lucide/svelte/icons/x-circle';
  import AlertTriangleIcon from '@lucide/svelte/icons/alert-triangle';
  import RefreshCwIcon from '@lucide/svelte/icons/refresh-cw';
  import Loader2Icon from '@lucide/svelte/icons/loader-2';
  import BotIcon from '@lucide/svelte/icons/bot';
  import CpuIcon from '@lucide/svelte/icons/cpu';
  import WrenchIcon from '@lucide/svelte/icons/wrench';
  import GitBranchIcon from '@lucide/svelte/icons/git-branch';
  import ArrowDownIcon from '@lucide/svelte/icons/arrow-down';
  import GitForkIcon from '@lucide/svelte/icons/git-fork';
  import ZapIcon from '@lucide/svelte/icons/zap';
  import BugIcon from '@lucide/svelte/icons/bug';
  import ShieldCheckIcon from '@lucide/svelte/icons/shield-check';
  import BookOpenIcon from '@lucide/svelte/icons/book-open';
  import InfinityIcon from '@lucide/svelte/icons/infinity';
  import HammerIcon from '@lucide/svelte/icons/hammer';
  import LayoutDashboardIcon from '@lucide/svelte/icons/layout-dashboard';
  import CodeIcon from '@lucide/svelte/icons/code';
  import ClipboardListIcon from '@lucide/svelte/icons/clipboard-list';
  import UsersIcon from '@lucide/svelte/icons/users';
  import SearchIcon from '@lucide/svelte/icons/search';
  import PaletteIcon from '@lucide/svelte/icons/palette';
  import FileTextIcon from '@lucide/svelte/icons/file-text';

  import { getClientOrNull } from '$lib/api/client';

  onMount(() => {
    // Root layout bootstrap is async — wait for API client to be ready
    let timer: ReturnType<typeof setInterval> | undefined;
    const tryFetch = () => {
      if (getClientOrNull()) {
        clearInterval(timer);
        codingPack.fetchStatus();
      }
    };
    tryFetch(); // Try immediately
    timer = setInterval(tryFetch, 300);
    return () => clearInterval(timer);
  });

  // Icon mapping
  const iconMap: Record<string, typeof ZapIcon> = {
    'zap': ZapIcon,
    'cpu': CpuIcon,
    'book-open': BookOpenIcon,
    'bug': BugIcon,
    'refresh-cw': RefreshCwIcon,
    'shield-check': ShieldCheckIcon,
    'wrench': WrenchIcon,
    'hammer': HammerIcon,
    'infinity': InfinityIcon
  };

  const agentIconMap: Record<string, typeof BotIcon> = {
    'architect': LayoutDashboardIcon,
    'dev': CodeIcon,
    'pm': ClipboardListIcon,
    'qa': ShieldCheckIcon,
    'sm': UsersIcon,
    'quick-flow-solo-dev': ZapIcon,
    'analyst': SearchIcon,
    'ux-designer': PaletteIcon,
    'tech-writer': FileTextIcon
  };

  function getStepIcon(type: string) {
    return type === 'agent' ? BotIcon : WrenchIcon;
  }

  let activeTab = $state<'coding' | 'bootstrap'>('coding');

  const codingWorkflows = WORKFLOWS.filter(w => w.category === 'coding');
  const bootstrapWorkflows = WORKFLOWS.filter(w => w.category === 'bootstrap');
  const activeWorkflows = $derived(activeTab === 'coding' ? codingWorkflows : bootstrapWorkflows);

  let expandedWorkflow = $state<string | null>(null);

  function toggleExpand(id: string): void {
    expandedWorkflow = expandedWorkflow === id ? null : id;
  }

  function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / 1048576).toFixed(1)} MB`;
  }

  function formatTime(iso: string): string {
    return new Date(iso).toLocaleString();
  }

  function getStateColor(state: string): string {
    const map: Record<string, string> = {
      Completed: '#10b981', Failed: '#ef4444', Running: '#3b82f6',
      Pending: '#f59e0b', HumanReview: '#8b5cf6', AwaitingHuman: '#8b5cf6'
    };
    return map[state] ?? '#64748b';
  }

  /** Check if a parallel group exists (multiple steps with same deps) */
  function getParallelSteps(wf: WorkflowMeta): Map<string, typeof wf.steps> {
    const groups = new Map<string, typeof wf.steps>();
    for (const step of wf.steps) {
      const key = step.depends_on.sort().join(',');
      const existing = groups.get(key) ?? [];
      existing.push(step);
      groups.set(key, existing);
    }
    return groups;
  }
</script>

<svelte:head>
  <title>Coding Pack - Pulse Dashboard</title>
</svelte:head>

<div class="coding-pack-page">
  <!-- Header -->
  <header class="page-header">
    <div class="header-left">
      <h1>
        <PackageIcon size={28} />
        Coding Pack
      </h1>
      <p class="header-desc">
        AI-driven software development workflows powered by BMAD methodology
      </p>
    </div>
    <Button variant="outline" size="sm" onclick={() => codingPack.fetchStatus()} disabled={codingPack.loading}>
      {#if codingPack.loading}
        <Loader2Icon size={16} class="animate-spin" />
      {:else}
        <RefreshCwIcon size={16} />
      {/if}
      Refresh
    </Button>
  </header>

  <!-- Status overview -->
  <section class="status-section">
    <div class="status-grid">
      <!-- Pack health -->
      <div class="status-card" class:healthy={codingPack.isHealthy} class:unhealthy={codingPack.packStatus && !codingPack.isHealthy}>
        <div class="status-icon">
          {#if codingPack.loading && !codingPack.packStatus}
            <Loader2Icon size={24} class="animate-spin" />
          {:else if codingPack.isHealthy}
            <CheckCircle2Icon size={24} />
          {:else if codingPack.packStatus}
            <XCircleIcon size={24} />
          {:else}
            <AlertTriangleIcon size={24} />
          {/if}
        </div>
        <div class="status-text">
          <span class="status-label">Pack Status</span>
          <span class="status-value">
            {#if codingPack.loading && !codingPack.packStatus}
              Checking...
            {:else if codingPack.isHealthy}
              Healthy
            {:else if codingPack.error}
              {codingPack.error}
            {:else}
              Degraded
            {/if}
          </span>
        </div>
      </div>

      <!-- Plugin count -->
      <div class="stat-card">
        <span class="stat-number">{codingPack.pluginCount}</span>
        <span class="stat-label">Plugins Installed</span>
      </div>

      <!-- Workflow count -->
      <div class="stat-card">
        <span class="stat-number">{codingPack.workflowCount}</span>
        <span class="stat-label">Workflows</span>
      </div>

      <!-- Agent count -->
      <div class="stat-card">
        <span class="stat-number">{AGENTS.length}</span>
        <span class="stat-label">AI Agents</span>
      </div>
    </div>

    <!-- Issues -->
    {#if codingPack.issues.length > 0}
      <div class="issues-banner">
        <AlertTriangleIcon size={16} />
        <div class="issues-content">
          {#each codingPack.issues as issue}
            <p>{issue}</p>
          {/each}
        </div>
      </div>
    {/if}
  </section>

  <!-- Installed Plugins -->
  {#if codingPack.installedPlugins.length > 0}
    <section class="plugins-section">
      <h2 class="section-title">
        <CpuIcon size={18} />
        Installed Plugins
      </h2>
      <div class="plugin-chips">
        {#each codingPack.installedPlugins as plugin}
          <div class="plugin-chip" class:ok={plugin.executable} class:fail={!plugin.executable}>
            {#if plugin.executable}
              <CheckCircle2Icon size={14} />
            {:else}
              <XCircleIcon size={14} />
            {/if}
            <span class="plugin-chip-name">{plugin.name}</span>
            {#if plugin.size_bytes > 0}
              <span class="plugin-chip-size">{formatBytes(plugin.size_bytes)}</span>
            {/if}
          </div>
        {/each}
      </div>
    </section>
  {/if}

  <!-- Workflows -->
  <section class="workflows-section">
    <div class="workflows-header">
      <h2 class="section-title">
        <GitBranchIcon size={18} />
        Workflows
      </h2>
      <div class="tab-bar">
        <button class="tab" class:active={activeTab === 'coding'} onclick={() => (activeTab = 'coding')}>
          Coding ({codingWorkflows.length})
        </button>
        <button class="tab" class:active={activeTab === 'bootstrap'} onclick={() => (activeTab = 'bootstrap')}>
          Bootstrap ({bootstrapWorkflows.length})
        </button>
      </div>
    </div>

    <div class="workflow-list">
      {#each activeWorkflows as wf (wf.id)}
        {@const WfIcon = iconMap[wf.icon] ?? CpuIcon}
        {@const tasks = codingPack.getWorkflowTasks(wf.id)}
        {@const isExpanded = expandedWorkflow === wf.id}
        <div class="wf-card" class:expanded={isExpanded} style="--wf-color: {wf.color}">
          <!-- Card header -->
          <button class="wf-header" onclick={() => toggleExpand(wf.id)}>
            <div class="wf-header-left">
              <div class="wf-icon">
                <WfIcon size={20} />
              </div>
              <div class="wf-info">
                <span class="wf-name">{wf.id}</span>
                <span class="wf-desc">{wf.description}</span>
              </div>
            </div>
            <div class="wf-header-right">
              <Badge variant="secondary" class="text-xs">{wf.steps.length} steps</Badge>
              {#if tasks.length > 0}
                <Badge variant="outline" class="text-xs">{tasks.length} runs</Badge>
              {/if}
              <ExecuteWorkflowDialog workflowId={wf.id} />
            </div>
          </button>

          <!-- Expanded content -->
          {#if isExpanded}
            <div class="wf-detail">
              <!-- Required plugins -->
              <div class="wf-requires">
                <span class="detail-label">Requires:</span>
                {#each wf.requires as req}
                  <span class="require-tag">{req}</span>
                {/each}
              </div>

              <!-- Step pipeline -->
              <div class="step-pipeline">
                {#each wf.steps as step, i (step.id)}
                  {@const StepIcon = getStepIcon(step.type)}
                  {@const isParallel = wf.steps.filter(s =>
                    s.depends_on.sort().join(',') === step.depends_on.sort().join(',')
                  ).length > 1}

                  {#if i > 0 && !isParallel}
                    <div class="pipeline-arrow">
                      <ArrowDownIcon size={14} />
                    </div>
                  {/if}

                  <div class="step-box" class:agent={step.type === 'agent'} class:fn={step.type === 'function'}
                       class:parallel={isParallel && i > 0 && wf.steps[i-1]?.depends_on.sort().join(',') === step.depends_on.sort().join(',')}>
                    {#if isParallel && i > 0 && wf.steps[i-1]?.depends_on.sort().join(',') !== step.depends_on.sort().join(',')}
                      <div class="parallel-marker">
                        <GitForkIcon size={10} />
                        parallel
                      </div>
                    {/if}
                    <div class="step-top">
                      <StepIcon size={14} />
                      <span class="step-id">{step.id}</span>
                      <span class="step-type">{step.type === 'agent' ? 'AI' : 'Fn'}</span>
                    </div>
                    {#if step.executor}
                      <span class="step-executor">{step.executor}</span>
                    {/if}
                    {#if step.config?.model_tier}
                      <span class="step-model">{step.config.model_tier}</span>
                    {/if}
                    {#if step.config?.context_from && step.config.context_from.length > 0}
                      <span class="step-context">ctx: {step.config.context_from.join(', ')}</span>
                    {/if}
                  </div>
                {/each}
              </div>

              <!-- Recent tasks for this workflow -->
              {#if tasks.length > 0}
                <div class="wf-tasks">
                  <span class="detail-label">Recent Executions:</span>
                  <div class="task-list">
                    {#each tasks.slice(0, 5) as task (task.id)}
                      <a href="/?task={task.id}" class="task-item">
                        <span class="task-id">{task.id.slice(0, 10)}…</span>
                        <span class="task-step">{task.step_id}</span>
                        <span class="task-state" style="color: {getStateColor(task.state)}">{task.state}</span>
                        <span class="task-time">{formatTime(task.created_at)}</span>
                      </a>
                    {/each}
                  </div>
                </div>
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  </section>

  <!-- AI Team -->
  <section class="team-section">
    <h2 class="section-title">
      <BotIcon size={18} />
      BMAD AI Team
    </h2>
    <div class="agent-grid">
      {#each AGENTS as agent (agent.id)}
        {@const AgentIcon = agentIconMap[agent.id] ?? BotIcon}
        <div class="agent-card" style="--agent-color: {agent.color}">
          <div class="agent-avatar">
            <AgentIcon size={20} />
          </div>
          <div class="agent-info">
            <span class="agent-name">{agent.name}</span>
            <span class="agent-role">{agent.roleZh}</span>
          </div>
          <span class="agent-id">bmad/{agent.id}</span>
        </div>
      {/each}
    </div>
  </section>
</div>

<style>
  .coding-pack-page {
    max-width: 1400px;
    margin: 0 auto;
  }

  /* Header */
  .page-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    margin-bottom: 1.5rem;
    gap: 1rem;
  }

  .header-left h1 {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 1.75rem;
    font-weight: 700;
    color: var(--pulse-text);
    margin: 0 0 0.25rem 0;
  }

  .header-desc {
    font-size: 0.9375rem;
    color: var(--pulse-text-secondary);
    margin: 0;
  }

  /* Section title */
  .section-title {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 1rem;
    font-weight: 600;
    color: var(--pulse-text);
    margin: 0 0 0.75rem 0;
  }

  .section-title :global(svg) {
    color: var(--pulse-primary);
  }

  /* Status section */
  .status-section {
    margin-bottom: 1.5rem;
  }

  .status-grid {
    display: grid;
    grid-template-columns: 2fr 1fr 1fr 1fr;
    gap: 0.75rem;
  }

  .status-card {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 1rem 1.25rem;
    background: var(--pulse-surface);
    border: 1px solid var(--pulse-border);
    border-radius: var(--radius-lg, 0.75rem);
  }

  .status-card.healthy .status-icon { color: var(--pulse-success); }
  .status-card.unhealthy .status-icon { color: var(--pulse-error); }

  .status-icon {
    flex-shrink: 0;
    color: var(--pulse-text-secondary);
  }

  .status-text {
    display: flex;
    flex-direction: column;
  }

  .status-label {
    font-size: 0.6875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--pulse-text-secondary);
  }

  .status-value {
    font-size: 0.9375rem;
    font-weight: 600;
    color: var(--pulse-text);
  }

  .stat-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 1rem;
    background: var(--pulse-surface);
    border: 1px solid var(--pulse-border);
    border-radius: var(--radius-lg, 0.75rem);
    gap: 0.25rem;
  }

  .stat-number {
    font-size: 1.75rem;
    font-weight: 700;
    color: var(--pulse-text);
    font-variant-numeric: tabular-nums;
    line-height: 1;
  }

  .stat-label {
    font-size: 0.6875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--pulse-text-secondary);
  }

  .issues-banner {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    padding: 0.75rem 1rem;
    background: oklch(0.25 0.08 80 / 0.3);
    border: 1px solid oklch(0.4 0.08 80 / 0.3);
    border-radius: var(--radius-md, 0.5rem);
    margin-top: 0.75rem;
    color: var(--pulse-warning);
    font-size: 0.8125rem;
  }

  .issues-banner :global(svg) { flex-shrink: 0; margin-top: 1px; }
  .issues-content p { margin: 0.125rem 0; }

  /* Plugins section */
  .plugins-section {
    margin-bottom: 1.5rem;
  }

  .plugin-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .plugin-chip {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    padding: 0.5rem 0.75rem;
    background: var(--pulse-surface);
    border: 1px solid var(--pulse-border);
    border-radius: var(--radius-md, 0.5rem);
    font-size: 0.8125rem;
  }

  .plugin-chip.ok :global(svg) { color: var(--pulse-success); }
  .plugin-chip.fail :global(svg) { color: var(--pulse-error); }
  .plugin-chip.fail { border-color: var(--pulse-error); }

  .plugin-chip-name {
    font-family: 'SF Mono', Monaco, Consolas, monospace;
    font-size: 0.75rem;
    color: var(--pulse-text);
  }

  .plugin-chip-size {
    font-size: 0.6875rem;
    color: var(--pulse-text-secondary);
  }

  /* Workflows section */
  .workflows-section {
    margin-bottom: 1.5rem;
  }

  .workflows-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.75rem;
  }

  .workflows-header .section-title {
    margin-bottom: 0;
  }

  .tab-bar {
    display: flex;
    gap: 0.25rem;
    background: var(--pulse-bg);
    border: 1px solid var(--pulse-border);
    border-radius: var(--radius-md, 0.5rem);
    padding: 0.1875rem;
  }

  .tab {
    padding: 0.375rem 0.75rem;
    border: none;
    border-radius: 0.375rem;
    background: transparent;
    font-size: 0.8125rem;
    font-weight: 500;
    color: var(--pulse-text-secondary);
    cursor: pointer;
    transition: all 0.15s;
  }

  .tab:hover { color: var(--pulse-text); }
  .tab.active {
    background: var(--pulse-surface);
    color: var(--pulse-text);
    box-shadow: 0 1px 2px rgba(0,0,0,0.1);
  }

  .workflow-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .wf-card {
    background: var(--pulse-surface);
    border: 1px solid var(--pulse-border);
    border-radius: var(--radius-lg, 0.75rem);
    border-left: 3px solid var(--wf-color);
    overflow: hidden;
    transition: border-color 0.15s;
  }

  .wf-card:hover {
    border-color: var(--wf-color);
  }

  .wf-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.875rem 1rem;
    width: 100%;
    border: none;
    background: transparent;
    cursor: pointer;
    text-align: left;
    gap: 0.75rem;
  }

  .wf-header-left {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    overflow: hidden;
    flex: 1;
  }

  .wf-icon {
    width: 2.25rem;
    height: 2.25rem;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-md, 0.5rem);
    background: color-mix(in srgb, var(--wf-color) 12%, transparent);
    color: var(--wf-color);
    flex-shrink: 0;
  }

  .wf-info {
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .wf-name {
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--pulse-text);
    font-family: 'SF Mono', Monaco, Consolas, monospace;
  }

  .wf-desc {
    font-size: 0.75rem;
    color: var(--pulse-text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .wf-header-right {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
  }

  /* Expanded detail */
  .wf-detail {
    padding: 0 1rem 1rem;
    border-top: 1px solid var(--pulse-border);
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .detail-label {
    font-size: 0.6875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--pulse-text-secondary);
    margin-right: 0.5rem;
  }

  .wf-requires {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.375rem;
    padding-top: 0.75rem;
  }

  .require-tag {
    font-size: 0.6875rem;
    font-family: 'SF Mono', Monaco, Consolas, monospace;
    padding: 0.125rem 0.5rem;
    background: var(--pulse-bg);
    border: 1px solid var(--pulse-border);
    border-radius: 0.25rem;
    color: var(--pulse-text);
  }

  /* Step pipeline */
  .step-pipeline {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0;
    padding-left: 0.5rem;
  }

  .pipeline-arrow {
    display: flex;
    align-items: center;
    padding: 0.125rem 0 0.125rem 1.5rem;
    color: var(--pulse-text-secondary);
    opacity: 0.3;
  }

  .step-box {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.375rem;
    padding: 0.375rem 0.625rem;
    background: var(--pulse-bg);
    border: 1px solid var(--pulse-border);
    border-radius: var(--radius-md, 0.5rem);
    width: fit-content;
    min-width: 200px;
  }

  .step-box.agent { border-left: 2px solid var(--pulse-primary); }
  .step-box.fn { border-left: 2px solid var(--pulse-text-secondary); }

  .step-top {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    width: 100%;
  }

  .step-top :global(svg) {
    color: var(--pulse-primary);
    flex-shrink: 0;
  }

  .step-box.fn .step-top :global(svg) {
    color: var(--pulse-text-secondary);
  }

  .step-id {
    font-size: 0.8125rem;
    font-weight: 600;
    color: var(--pulse-text);
    font-family: 'SF Mono', Monaco, Consolas, monospace;
  }

  .step-type {
    font-size: 0.5625rem;
    font-weight: 700;
    padding: 0.0625rem 0.25rem;
    border-radius: 0.1875rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-left: auto;
  }

  .step-box.agent .step-type {
    background: oklch(0.3 0.1 250 / 0.3);
    color: oklch(0.7 0.1 250);
  }

  .step-box.fn .step-type {
    background: oklch(0.3 0 0 / 0.5);
    color: oklch(0.6 0 0);
  }

  .step-executor, .step-model, .step-context {
    font-size: 0.625rem;
    font-family: 'SF Mono', Monaco, Consolas, monospace;
    color: var(--pulse-text-secondary);
  }

  .parallel-marker {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.5625rem;
    font-weight: 600;
    text-transform: uppercase;
    color: var(--pulse-warning);
    margin-bottom: 0.125rem;
  }

  /* Tasks */
  .wf-tasks {
    border-top: 1px solid var(--pulse-border);
    padding-top: 0.75rem;
  }

  .task-list {
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
    margin-top: 0.375rem;
  }

  .task-item {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.375rem 0.5rem;
    border-radius: 0.25rem;
    text-decoration: none;
    font-size: 0.75rem;
    transition: background 0.15s;
  }

  .task-item:hover { background: var(--pulse-bg); }

  .task-id {
    font-family: 'SF Mono', Monaco, Consolas, monospace;
    color: var(--pulse-text);
    min-width: 7rem;
  }

  .task-step {
    color: var(--pulse-text-secondary);
    min-width: 5rem;
  }

  .task-state {
    font-weight: 600;
    text-transform: uppercase;
    font-size: 0.625rem;
    letter-spacing: 0.03em;
  }

  .task-time {
    color: var(--pulse-text-secondary);
    margin-left: auto;
    font-size: 0.6875rem;
  }

  /* Team section */
  .team-section {
    margin-bottom: 2rem;
  }

  .agent-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 0.5rem;
  }

  .agent-card {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem 1rem;
    background: var(--pulse-surface);
    border: 1px solid var(--pulse-border);
    border-radius: var(--radius-md, 0.5rem);
    transition: border-color 0.15s;
  }

  .agent-card:hover {
    border-color: var(--agent-color);
  }

  .agent-avatar {
    width: 2.25rem;
    height: 2.25rem;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    background: color-mix(in srgb, var(--agent-color) 15%, transparent);
    color: var(--agent-color);
    flex-shrink: 0;
  }

  .agent-info {
    display: flex;
    flex-direction: column;
    flex: 1;
    overflow: hidden;
  }

  .agent-name {
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--pulse-text);
  }

  .agent-role {
    font-size: 0.75rem;
    color: var(--pulse-text-secondary);
  }

  .agent-id {
    font-size: 0.625rem;
    font-family: 'SF Mono', Monaco, Consolas, monospace;
    color: var(--pulse-text-secondary);
    opacity: 0;
    transition: opacity 0.15s;
    white-space: nowrap;
  }

  .agent-card:hover .agent-id { opacity: 1; }

  /* Responsive */
  @media (max-width: 767px) {
    .page-header { flex-direction: column; }
    .header-left h1 { font-size: 1.375rem; }

    .status-grid {
      grid-template-columns: 1fr 1fr;
    }

    .workflows-header {
      flex-direction: column;
      align-items: flex-start;
      gap: 0.5rem;
    }

    .wf-header {
      flex-direction: column;
      align-items: flex-start;
    }

    .wf-header-right {
      margin-top: 0.5rem;
    }

    .agent-grid {
      grid-template-columns: 1fr 1fr;
    }
  }
</style>
