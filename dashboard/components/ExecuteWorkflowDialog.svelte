<script lang="ts">
  import { goto } from '$app/navigation';
  import { getClientOrNull } from '$lib/api/client';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import PlayIcon from '@lucide/svelte/icons/play';
  import LoaderIcon from '@lucide/svelte/icons/loader-2';
  import XIcon from '@lucide/svelte/icons/x';
  import InfoIcon from '@lucide/svelte/icons/info';

  interface Props {
    workflowId: string;
  }

  let { workflowId }: Props = $props();

  let executing = $state(false);
  let dialogOpen = $state(false);
  let resultMessage = $state<string | null>(null);
  let errorMessage = $state<string | null>(null);
  let inputText = $state('');

  /** Workflow-specific hints and input mode */
  const workflowMeta = $derived.by(() => {
    const id = workflowId.toLowerCase();
    const meta: Record<string, { hint: string; placeholder: string; field: string }> = {
      'coding-quick-dev': {
        hint: '小功能、快速修改 (3 steps: spec → implement → commit)',
        placeholder: '在 login endpoint 加上 input validation',
        field: 'input'
      },
      'coding-feature-dev': {
        hint: '完整功能開發 (5 steps: architect → worktree → dev → QA → commit)',
        placeholder: '實作使用者通知系統，支援 email 和 in-app 兩種管道',
        field: 'input'
      },
      'coding-story-dev': {
        hint: 'User Story 驅動開發 (6 steps: SM → architect → worktree → dev → QA → commit)',
        placeholder: '身為使用者，我希望能匯出 CSV 報表以便離線分析',
        field: 'input'
      },
      'coding-bug-fix': {
        hint: 'Bug 修復 (4 steps: analyze → fix → edge-case review → commit)',
        placeholder: '當 user_id 為 null 時 /api/profile 回傳 500 而非 404',
        field: 'input'
      },
      'coding-refactor': {
        hint: '安全重構 (4 steps: plan → execute → regression → commit)',
        placeholder: '將 UserService 的 database 操作抽成 Repository pattern',
        field: 'input'
      },
      'coding-review': {
        hint: '多層次 Code Review (3 steps: adversarial + edge-case → synthesis)',
        placeholder: 'src/auth/',
        field: 'target'
      },
      'bootstrap-plugin': {
        hint: '開發單一 Plugin (5 steps: plan → implement → test → QA → commit)',
        placeholder: '為 validator 加上 step dependency 循環檢測',
        field: 'input'
      },
      'bootstrap-rebuild': {
        hint: '重建所有 Plugins (3 steps: build → install → validate)',
        placeholder: '',
        field: 'input'
      },
      'bootstrap-cycle': {
        hint: '完整自我演進 (8 steps: plan → implement → test → review → rebuild → install → validate → commit)',
        placeholder: '重構 pack.rs 的錯誤處理，改用 thiserror',
        field: 'input'
      }
    };
    return meta[id] ?? { hint: '', placeholder: 'Describe the task...', field: 'input' };
  });

  function openDialog(): void {
    dialogOpen = true;
    resultMessage = null;
    errorMessage = null;
    inputText = '';
  }

  function closeDialog(): void {
    dialogOpen = false;
  }

  function focusOnMount(node: HTMLElement): void {
    node.focus();
  }

  function handleOverlayClick(e: MouseEvent): void {
    if ((e.target as HTMLElement)?.classList?.contains('dialog-overlay')) {
      closeDialog();
    }
  }

  function handleKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape') closeDialog();
    if (e.key === 'Enter' && e.ctrlKey && inputText.trim()) execute();
  }

  async function execute(): Promise<void> {
    const client = getClientOrNull();
    if (!client) return;

    try {
      executing = true;
      errorMessage = null;

      const inputs: Record<string, unknown> = {};
      if (inputText.trim()) {
        inputs[workflowMeta.field] = inputText.trim();
      }

      const result = await client.executeWorkflow(
        workflowId,
        Object.keys(inputs).length > 0 ? inputs : undefined
      );
      resultMessage = `Workflow triggered — Task ${result.task_id}`;
      dialogOpen = false;

      setTimeout(() => {
        goto(`/?task=${result.task_id}`);
      }, 1500);
    } catch (err) {
      errorMessage = err instanceof Error ? err.message : 'Execution failed';
    } finally {
      executing = false;
    }
  }
</script>

<Button variant="default" size="sm" onclick={openDialog}>
  <PlayIcon size={16} />
  Execute
</Button>

{#if dialogOpen}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions a11y_interactive_supports_focus -->
  <div
    class="dialog-overlay"
    role="dialog"
    aria-modal="true"
    aria-labelledby="exec-dialog-title"
    onclick={handleOverlayClick}
    onkeydown={handleKeydown}
    tabindex="-1"
    use:focusOnMount
  >
    <div class="dialog-content">
      <div class="dialog-header">
        <h2 id="exec-dialog-title">Execute {workflowId}</h2>
        <button class="close-btn" onclick={closeDialog} aria-label="Close">
          <XIcon size={16} />
        </button>
      </div>

      {#if workflowMeta.hint}
        <div class="dialog-hint">
          <InfoIcon size={14} />
          <span>{workflowMeta.hint}</span>
        </div>
      {/if}

      <div class="input-section">
        <label for="workflow-input" class="input-label">
          {workflowMeta.field === 'target' ? 'Target Path' : 'Task Description'}
        </label>
        <textarea
          id="workflow-input"
          class="input-textarea"
          placeholder={workflowMeta.placeholder}
          bind:value={inputText}
          rows="3"
          disabled={executing}
        ></textarea>
        <span class="input-hint">Ctrl+Enter to execute</span>
      </div>

      {#if errorMessage}
        <div class="dialog-error" role="alert">{errorMessage}</div>
      {/if}

      <div class="dialog-actions">
        <Button variant="outline" onclick={closeDialog} disabled={executing}>Cancel</Button>
        <Button variant="default" onclick={execute} disabled={executing || !inputText.trim()}>
          {#if executing}
            <LoaderIcon size={16} class="animate-spin" />
            Executing...
          {:else}
            <PlayIcon size={16} />
            Execute
          {/if}
        </Button>
      </div>
    </div>
  </div>
{/if}

{#if resultMessage}
  <div class="toast-success" role="status">
    {resultMessage}
  </div>
{/if}

<style>
  .dialog-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 50;
    backdrop-filter: blur(2px);
  }

  .dialog-content {
    background: var(--pulse-bg-elevated, oklch(0.15 0 0));
    border: 1px solid var(--pulse-border, oklch(0.3 0 0));
    border-radius: 0.75rem;
    padding: 1.5rem;
    max-width: 32rem;
    width: 90%;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
  }

  .dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.75rem;
  }

  .dialog-header h2 {
    font-size: 1.125rem;
    font-weight: 600;
    color: var(--pulse-text, oklch(0.9 0 0));
    margin: 0;
    font-family: 'SF Mono', Monaco, Consolas, monospace;
  }

  .close-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border: none;
    border-radius: var(--radius-sm, 0.25rem);
    background: transparent;
    color: var(--pulse-text-secondary);
    cursor: pointer;
    transition: all 0.15s;
  }

  .close-btn:hover {
    color: var(--pulse-text);
    background: var(--pulse-bg);
  }

  .dialog-hint {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    padding: 0.625rem 0.75rem;
    background: oklch(0.25 0.05 230 / 0.3);
    border: 1px solid oklch(0.35 0.05 230 / 0.3);
    border-radius: 0.5rem;
    font-size: 0.8125rem;
    color: oklch(0.75 0.05 230);
    margin-bottom: 1rem;
    line-height: 1.4;
  }

  .dialog-hint :global(svg) {
    flex-shrink: 0;
    margin-top: 1px;
  }

  .input-section {
    margin-bottom: 1rem;
  }

  .input-label {
    display: block;
    font-size: 0.8125rem;
    font-weight: 600;
    color: var(--pulse-text-secondary);
    margin-bottom: 0.375rem;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .input-textarea {
    width: 100%;
    padding: 0.625rem 0.75rem;
    background: var(--pulse-bg, oklch(0.12 0 0));
    border: 1px solid var(--pulse-border, oklch(0.3 0 0));
    border-radius: 0.5rem;
    color: var(--pulse-text, oklch(0.9 0 0));
    font-size: 0.875rem;
    font-family: inherit;
    resize: vertical;
    min-height: 4rem;
    transition: border-color 0.15s;
    box-sizing: border-box;
  }

  .input-textarea:focus {
    outline: none;
    border-color: var(--pulse-primary, oklch(0.6 0.18 250));
    box-shadow: 0 0 0 2px oklch(0.6 0.18 250 / 0.15);
  }

  .input-textarea::placeholder {
    color: oklch(0.45 0 0);
  }

  .input-hint {
    display: block;
    font-size: 0.6875rem;
    color: var(--pulse-text-secondary);
    margin-top: 0.25rem;
    text-align: right;
  }

  .dialog-error {
    background: oklch(0.3 0.1 25 / 0.3);
    color: oklch(0.8 0.1 25);
    padding: 0.5rem 0.75rem;
    border-radius: 0.375rem;
    font-size: 0.8125rem;
    margin-bottom: 1rem;
  }

  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }

  .toast-success {
    position: fixed;
    bottom: 1.5rem;
    right: 1.5rem;
    background: oklch(0.25 0.1 150);
    color: oklch(0.85 0.1 150);
    padding: 0.75rem 1rem;
    border-radius: 0.5rem;
    font-size: 0.875rem;
    z-index: 100;
    animation: fadeIn 0.2s ease;
  }

  @keyframes fadeIn {
    from { opacity: 0; transform: translateY(0.5rem); }
    to { opacity: 1; transform: translateY(0); }
  }
</style>
