import { getClientOrNull } from '$lib/api/client';
import type { TaskResponse } from '$lib/types/api';

/** Plugin binary info from list-plugins action */
export interface PackPlugin {
  name: string;
  size_bytes: number;
  executable: boolean;
}

/** Validation result from validate-pack action */
export interface PackValidation {
  valid: boolean;
  plugins_ok: number;
  workflows_found: number;
  issues: string[];
}

/** Full status from status action */
export interface PackStatusResponse {
  validation: PackValidation;
  workflows: { workflows: string[]; count: number };
  plugins: { plugins: PackPlugin[]; count: number };
}

/** Workflow step definition matching YAML structure */
export interface WorkflowStep {
  id: string;
  type: 'agent' | 'function';
  depends_on: string[];
  executor?: string;
  config?: {
    model_tier?: string;
    system_prompt?: string;
    user_prompt_template?: string;
    max_tokens?: number;
    context_from?: string[];
    command?: string[];
    timeout_seconds?: number;
  };
}

/** Local workflow metadata enriched from YAML definitions */
export interface WorkflowMeta {
  id: string;
  description: string;
  category: 'coding' | 'bootstrap';
  steps: WorkflowStep[];
  requires: string[];
  inputField: 'input' | 'target';
  placeholder: string;
  icon: string;
  color: string;
}

/** BMAD agent definition */
export interface BmadAgent {
  id: string;
  name: string;
  role: string;
  roleZh: string;
  color: string;
}

// ============================================================================
// Static workflow definitions from YAML
// ============================================================================

export const WORKFLOWS: WorkflowMeta[] = [
  {
    id: 'coding-quick-dev',
    description: 'Quick development: rapid spec → implement → commit',
    category: 'coding',
    inputField: 'input',
    placeholder: '在 login endpoint 加上 input validation',
    icon: 'zap',
    color: '#10b981',
    requires: ['provider-claude-code', 'bmad-method', 'plugin-git-ops'],
    steps: [
      { id: 'quick_spec', type: 'agent', depends_on: [], executor: 'bmad-method', config: { model_tier: 'fast', max_tokens: 2048 } },
      { id: 'implement', type: 'agent', depends_on: ['quick_spec'], executor: 'provider-claude-code', config: { model_tier: 'balanced', max_tokens: 4096, context_from: ['quick_spec'] } },
      { id: 'git_commit', type: 'function', depends_on: ['implement'], config: { command: ['git', 'add', '-A'] } }
    ]
  },
  {
    id: 'coding-feature-dev',
    description: 'Full feature dev: architect → worktree → dev → QA → commit',
    category: 'coding',
    inputField: 'input',
    placeholder: '實作使用者通知系統，支援 email 和 in-app 兩種管道',
    icon: 'cpu',
    color: '#3b82f6',
    requires: ['provider-claude-code', 'bmad-method', 'plugin-git-ops', 'plugin-git-worktree'],
    steps: [
      { id: 'architect_design', type: 'agent', depends_on: [], executor: 'bmad-method', config: { model_tier: 'balanced', max_tokens: 4096 } },
      { id: 'create_worktree', type: 'function', depends_on: ['architect_design'], executor: 'plugin-git-worktree' },
      { id: 'dev_implement', type: 'agent', depends_on: ['create_worktree'], executor: 'provider-claude-code', config: { model_tier: 'balanced', max_tokens: 8192, context_from: ['architect_design'] } },
      { id: 'qa_review', type: 'agent', depends_on: ['dev_implement'], executor: 'bmad-method', config: { model_tier: 'balanced', max_tokens: 4096, context_from: ['dev_implement'] } },
      { id: 'git_commit', type: 'function', depends_on: ['qa_review'], config: { command: ['git', 'add', '-A'] } }
    ]
  },
  {
    id: 'coding-story-dev',
    description: 'Story-driven: SM prepares → architect → worktree → dev → QA → commit',
    category: 'coding',
    inputField: 'input',
    placeholder: '身為使用者，我希望能匯出 CSV 報表以便離線分析',
    icon: 'book-open',
    color: '#8b5cf6',
    requires: ['provider-claude-code', 'bmad-method', 'plugin-git-ops', 'plugin-git-worktree'],
    steps: [
      { id: 'prepare_story', type: 'agent', depends_on: [], executor: 'bmad-method', config: { model_tier: 'balanced', max_tokens: 4096 } },
      { id: 'technical_design', type: 'agent', depends_on: ['prepare_story'], executor: 'bmad-method', config: { model_tier: 'balanced', max_tokens: 4096, context_from: ['prepare_story'] } },
      { id: 'create_worktree', type: 'function', depends_on: ['technical_design'], executor: 'plugin-git-worktree' },
      { id: 'implement', type: 'agent', depends_on: ['create_worktree'], executor: 'provider-claude-code', config: { model_tier: 'balanced', max_tokens: 8192, context_from: ['prepare_story', 'technical_design'] } },
      { id: 'qa_review', type: 'agent', depends_on: ['implement'], executor: 'bmad-method', config: { model_tier: 'balanced', max_tokens: 4096, context_from: ['prepare_story', 'implement'] } },
      { id: 'git_commit', type: 'function', depends_on: ['qa_review'], config: { command: ['git', 'add', '-A'] } }
    ]
  },
  {
    id: 'coding-bug-fix',
    description: 'Bug fix: root cause analysis → fix → edge-case review → commit',
    category: 'coding',
    inputField: 'input',
    placeholder: '當 user_id 為 null 時 /api/profile 回傳 500 而非 404',
    icon: 'bug',
    color: '#ef4444',
    requires: ['provider-claude-code', 'bmad-method', 'plugin-git-ops'],
    steps: [
      { id: 'analyze_bug', type: 'agent', depends_on: [], executor: 'bmad-method', config: { model_tier: 'balanced', max_tokens: 4096 } },
      { id: 'implement_fix', type: 'agent', depends_on: ['analyze_bug'], executor: 'provider-claude-code', config: { model_tier: 'balanced', max_tokens: 4096, context_from: ['analyze_bug'] } },
      { id: 'edge_case_review', type: 'agent', depends_on: ['implement_fix'], executor: 'bmad-method', config: { model_tier: 'balanced', max_tokens: 4096, context_from: ['implement_fix'] } },
      { id: 'git_commit', type: 'function', depends_on: ['edge_case_review'], config: { command: ['git', 'add', '-A'] } }
    ]
  },
  {
    id: 'coding-refactor',
    description: 'Safe refactoring: plan → execute → regression check → commit',
    category: 'coding',
    inputField: 'input',
    placeholder: '將 UserService 的 database 操作抽成 Repository pattern',
    icon: 'refresh-cw',
    color: '#f59e0b',
    requires: ['provider-claude-code', 'bmad-method', 'plugin-git-ops'],
    steps: [
      { id: 'plan_refactor', type: 'agent', depends_on: [], executor: 'bmad-method', config: { model_tier: 'balanced', max_tokens: 4096 } },
      { id: 'execute_refactor', type: 'agent', depends_on: ['plan_refactor'], executor: 'provider-claude-code', config: { model_tier: 'balanced', max_tokens: 8192, context_from: ['plan_refactor'] } },
      { id: 'regression_check', type: 'agent', depends_on: ['execute_refactor'], executor: 'bmad-method', config: { model_tier: 'balanced', max_tokens: 4096, context_from: ['execute_refactor'] } },
      { id: 'git_commit', type: 'function', depends_on: ['regression_check'], config: { command: ['git', 'add', '-A'] } }
    ]
  },
  {
    id: 'coding-review',
    description: 'Multi-layer code review: adversarial + edge-case → synthesis',
    category: 'coding',
    inputField: 'target',
    placeholder: 'src/auth/',
    icon: 'shield-check',
    color: '#06b6d4',
    requires: ['bmad-method'],
    steps: [
      { id: 'adversarial_review', type: 'agent', depends_on: [], executor: 'bmad-method', config: { model_tier: 'balanced', max_tokens: 4096 } },
      { id: 'edge_case_review', type: 'agent', depends_on: [], executor: 'bmad-method', config: { model_tier: 'balanced', max_tokens: 4096 } },
      { id: 'review_synthesis', type: 'agent', depends_on: ['adversarial_review', 'edge_case_review'], executor: 'bmad-method', config: { model_tier: 'balanced', max_tokens: 4096, context_from: ['adversarial_review', 'edge_case_review'] } }
    ]
  },
  {
    id: 'bootstrap-plugin',
    description: 'Self-dev: plan → implement → test → QA → commit',
    category: 'bootstrap',
    inputField: 'input',
    placeholder: '為 validator 加上 step dependency 循環檢測',
    icon: 'wrench',
    color: '#f59e0b',
    requires: ['provider-claude-code', 'bmad-method', 'plugin-git-ops'],
    steps: [
      { id: 'architect_plan', type: 'agent', depends_on: [], executor: 'bmad-method', config: { model_tier: 'balanced', max_tokens: 4096 } },
      { id: 'implement', type: 'agent', depends_on: ['architect_plan'], executor: 'provider-claude-code', config: { model_tier: 'balanced', max_tokens: 8192, context_from: ['architect_plan'] } },
      { id: 'build_verify', type: 'function', depends_on: ['implement'], config: { command: ['cargo', 'test'], timeout_seconds: 300 } },
      { id: 'qa_review', type: 'agent', depends_on: ['build_verify'], executor: 'bmad-method', config: { model_tier: 'balanced', max_tokens: 4096, context_from: ['implement'] } },
      { id: 'git_commit', type: 'function', depends_on: ['qa_review'], config: { command: ['git', 'add', '-A'] } }
    ]
  },
  {
    id: 'bootstrap-rebuild',
    description: 'Rebuild all plugins: build → install → validate',
    category: 'bootstrap',
    inputField: 'input',
    placeholder: '',
    icon: 'hammer',
    color: '#a855f7',
    requires: ['provider-claude-code'],
    steps: [
      { id: 'build_all', type: 'function', depends_on: [], config: { timeout_seconds: 600 } },
      { id: 'install_plugins', type: 'function', depends_on: ['build_all'] },
      { id: 'validate', type: 'function', depends_on: ['install_plugins'] }
    ]
  },
  {
    id: 'bootstrap-cycle',
    description: 'Full self-evolution: plan → implement → test → review → rebuild → install → validate → commit',
    category: 'bootstrap',
    inputField: 'input',
    placeholder: '重構 pack.rs 的錯誤處理，改用 thiserror',
    icon: 'infinity',
    color: '#ec4899',
    requires: ['provider-claude-code', 'bmad-method', 'plugin-git-ops'],
    steps: [
      { id: 'plan', type: 'agent', depends_on: [], executor: 'bmad-method', config: { model_tier: 'balanced', max_tokens: 4096 } },
      { id: 'implement', type: 'agent', depends_on: ['plan'], executor: 'provider-claude-code', config: { model_tier: 'balanced', max_tokens: 8192, context_from: ['plan'] } },
      { id: 'test', type: 'function', depends_on: ['implement'], config: { command: ['cargo', 'test'], timeout_seconds: 300 } },
      { id: 'review', type: 'agent', depends_on: ['test'], executor: 'bmad-method', config: { model_tier: 'balanced', max_tokens: 4096, context_from: ['implement'] } },
      { id: 'rebuild', type: 'function', depends_on: ['review'], config: { timeout_seconds: 600 } },
      { id: 'install', type: 'function', depends_on: ['rebuild'] },
      { id: 'validate', type: 'function', depends_on: ['install'] },
      { id: 'commit', type: 'function', depends_on: ['validate'], config: { command: ['git', 'add', '-A'] } }
    ]
  }
];

/** Workspace-scoped workflows (coding-*) — operate on project code */
export const WORKSPACE_WORKFLOWS = WORKFLOWS.filter(w => w.category === 'coding');

/** System-level workflows (bootstrap-*) — operate on the Pulse platform itself */
export const SYSTEM_WORKFLOWS = WORKFLOWS.filter(w => w.category === 'bootstrap');

export const AGENTS: BmadAgent[] = [
  { id: 'architect', name: 'Winston', role: 'System Architect', roleZh: '系統架構師', color: '#3b82f6' },
  { id: 'dev', name: 'Amelia', role: 'Developer', roleZh: '開發者', color: '#10b981' },
  { id: 'pm', name: 'John', role: 'Product Manager', roleZh: '產品經理', color: '#8b5cf6' },
  { id: 'qa', name: 'Quinn', role: 'QA Engineer', roleZh: 'QA 工程師', color: '#f59e0b' },
  { id: 'sm', name: 'Bob', role: 'Scrum Master', roleZh: 'Scrum Master', color: '#06b6d4' },
  { id: 'quick-flow-solo-dev', name: 'Barry', role: 'Quick Flow Specialist', roleZh: '快速開發專家', color: '#ef4444' },
  { id: 'analyst', name: 'Mary', role: 'Business Analyst', roleZh: '商業分析師', color: '#ec4899' },
  { id: 'ux-designer', name: 'Sally', role: 'UX Designer', roleZh: 'UX 設計師', color: '#14b8a6' },
  { id: 'tech-writer', name: 'Paige', role: 'Technical Writer', roleZh: '技術寫手', color: '#a855f7' }
];

// ============================================================================
// Reactive store
// ============================================================================

class CodingPackStore {
  packStatus = $state<PackStatusResponse | null>(null);
  recentTasks = $state<TaskResponse[]>([]);
  loading = $state(false);
  error = $state<string | null>(null);
  selectedWorkflow = $state<string | null>(null);
  lastFetched = $state<Date | null>(null);

  get isHealthy(): boolean {
    return this.packStatus?.validation.valid ?? false;
  }

  get pluginCount(): number {
    return this.packStatus?.plugins.count ?? 0;
  }

  get workflowCount(): number {
    return this.packStatus?.workflows.count ?? 0;
  }

  get issues(): string[] {
    return this.packStatus?.validation.issues ?? [];
  }

  get installedPlugins(): PackPlugin[] {
    return this.packStatus?.plugins.plugins ?? [];
  }

  get registeredWorkflows(): string[] {
    return this.packStatus?.workflows.workflows ?? [];
  }

  getWorkflowTasks(workflowId: string): TaskResponse[] {
    return this.recentTasks.filter(t => t.workflow_id === workflowId);
  }

  async fetchStatus(): Promise<void> {
    const client = getClientOrNull();
    if (!client) return;

    this.loading = true;
    this.error = null;

    let usedPackApi = false;
    try {
      // Try the pack's status action (single attempt, no retries)
      const resp = await fetch('/api/v1/plugin-coding-pack/status', {
        headers: { 'Content-Type': 'application/json' },
        signal: AbortSignal.timeout(3000)
      });
      if (resp.ok) {
        const text = await resp.text();
        if (text.length > 2) {
          this.packStatus = JSON.parse(text) as PackStatusResponse;
          usedPackApi = true;
        }
      }
    } catch {
      // Pack API not available — fall through to standard API
    }

    if (!usedPackApi) {
      // Fallback: build status from standard API
      try {
        const [plugins, workflows] = await Promise.all([
          client.listPlugins().catch(() => []),
          client.listWorkflows().catch(() => [])
        ]);

        // Check which required coding-pack plugins are installed
        const requiredPlugins = ['bmad-method', 'provider-claude-code'];
        const optionalPlugins = ['git-ops', 'plugin-git-ops', 'plugin-git-worktree'];
        const installedNames = new Set(plugins.map(p => p.name));
        const issues: string[] = [];

        for (const req of requiredPlugins) {
          if (!installedNames.has(req)) {
            issues.push(`Required plugin '${req}' not found`);
          }
        }

        const requiredOk = requiredPlugins.every(r => installedNames.has(r));

        // Count coding-pack workflows specifically
        const codingWorkflowNames = WORKFLOWS.map(w => w.id);
        const registeredCodingWorkflows = workflows.filter(w => codingWorkflowNames.includes(w.id));
        const allWorkflowNames = [
          ...workflows.map(w => w.id),
          // Include static definitions for workflows not yet registered
          ...codingWorkflowNames.filter(n => !workflows.some(w => w.id === n))
        ];

        if (registeredCodingWorkflows.length === 0 && workflows.length > 0) {
          issues.push('Coding pack workflows not registered yet — run: pulse registry validate --config ./config');
        }

        this.packStatus = {
          validation: {
            valid: requiredOk,
            plugins_ok: plugins.length,
            workflows_found: workflows.length + (codingWorkflowNames.length - registeredCodingWorkflows.length),
            issues
          },
          workflows: {
            workflows: allWorkflowNames,
            count: allWorkflowNames.length
          },
          plugins: {
            plugins: plugins.map(p => ({
              name: p.name,
              size_bytes: 0,
              executable: true
            })),
            count: plugins.length
          }
        };
      } catch (err) {
        this.error = err instanceof Error ? err.message : 'Failed to fetch status';
      }
    }

    // Fetch recent tasks
    try {
      const client2 = getClientOrNull();
      if (client2) {
        const { items } = await client2.listTasks(100, 0);
        this.recentTasks = items;
      }
    } catch {
      // Non-critical, don't set error
    }

    this.loading = false;
    this.lastFetched = new Date();
  }

  selectWorkflow(id: string | null): void {
    this.selectedWorkflow = id;
  }
}

export const codingPack = new CodingPackStore();

// ============================================================================
// SDK data-format helpers — transform local data to SDK endpoint responses
// ============================================================================

/** Format workflows as SDK table items: GET /workflows/list → { items: [...] } */
export function workflowsAsTableItems(): { items: Record<string, unknown>[] } {
  return {
    items: WORKFLOWS.map(w => ({
      id: w.id,
      description: w.description,
      category: w.category,
      step_count: w.steps.length,
      requires: w.requires.join(', '),
      last_run: codingPack.getWorkflowTasks(w.id)[0]?.created_at ?? '—',
      icon: w.icon,
      color: w.color
    }))
  };
}

/** Format agents as SDK table items: GET /agents/list → { items: [...] } */
export function agentsAsTableItems(): { items: Record<string, unknown>[] } {
  // Count which workflows each agent participates in
  const agentWorkflows = new Map<string, string[]>();
  for (const wf of WORKFLOWS) {
    for (const step of wf.steps) {
      if (step.executor === 'bmad-method') {
        // All bmad-method steps use BMAD agents
        for (const agent of AGENTS) {
          const list = agentWorkflows.get(agent.id) ?? [];
          if (!list.includes(wf.id)) list.push(wf.id);
          agentWorkflows.set(agent.id, list);
        }
      }
    }
  }

  return {
    items: AGENTS.map(a => ({
      id: a.id,
      name: a.name,
      role: a.role,
      role_zh: a.roleZh,
      color: a.color,
      assigned_workflows: (agentWorkflows.get(a.id) ?? []).length
    }))
  };
}

/** Format workflow detail for SDK detail renderer: GET /workflows/{id} */
export function workflowAsDetail(workflowId: string): Record<string, unknown> | null {
  const wf = WORKFLOWS.find(w => w.id === workflowId);
  if (!wf) return null;

  const tasks = codingPack.getWorkflowTasks(wf.id);
  const completed = tasks.filter(t => t.state === 'Completed');

  return {
    id: wf.id,
    description: wf.description,
    category: wf.category,
    requires: wf.requires.join(', '),
    step_count: wf.steps.length,
    step_pipeline: wf.steps.map(s => `${s.id} (${s.type})`).join(' → '),
    parallel_groups: wf.steps.filter((s, i, arr) =>
      arr.some((other, j) => j !== i &&
        other.depends_on.sort().join(',') === s.depends_on.sort().join(','))
    ).length,
    last_run: tasks[0]?.created_at ?? '—',
    total_runs: tasks.length,
    success_rate: tasks.length > 0
      ? `${Math.round((completed.length / tasks.length) * 100)}%`
      : '—'
  };
}
