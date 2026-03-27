import { test, expect } from '@playwright/test';
import { initPage, goToPlugin, PLUGIN_BASE } from './helpers';

test.describe('Scrum Board — Board Page (P0 + P1)', () => {
  test.beforeEach(async ({ page }) => {
    await initPage(page);
  });

  // ── P0: Core Functionality ──

  test('P0-01: board page loads and renders 5 status columns', async ({ page }) => {
    await goToPlugin(page, '/board');
    const main = page.locator('main');
    for (const col of ['Backlog', 'Ready', 'In Progress', 'Review', 'Done']) {
      await expect(main.getByText(col, { exact: false }).first()).toBeVisible({ timeout: 5000 });
    }
  });

  test('P0-02: board page displays epic and story cards', async ({ page }) => {
    await goToPlugin(page, '/board');
    const main = page.locator('main');
    await expect(main.locator('[data-type="epic"]').first()).toBeVisible({ timeout: 5000 });
    await expect(main.locator('[data-type="story"]').first()).toBeVisible({ timeout: 5000 });
  });

  test('P0-03: summary bar shows totals and progress percentage', async ({ page }) => {
    await goToPlugin(page, '/board');
    const summary = page.locator('.board-summary');
    await expect(summary).toBeVisible({ timeout: 5000 });
    // Should display numeric values for epics, stories, done
    await expect(summary.locator('.stat-value').first()).toBeVisible();
    // Should display progress percentage
    await expect(summary.locator('.progress-text')).toContainText('%');
  });

  test('P0-06: sidebar shows Scrum Board nav link', async ({ page }) => {
    await goToPlugin(page, '/board');
    const link = page.locator(`aside a[href*="${PLUGIN_BASE}/board"]`);
    await expect(link).toBeVisible({ timeout: 5000 });
  });

  // ── P1: Critical Paths ──

  test('P1-05: swimlane headers are collapsible', async ({ page }) => {
    await goToPlugin(page, '/board');
    const swimlaneHeader = page.locator('.swimlane-header').first();
    // Skip if no swimlanes (flat board)
    if (await swimlaneHeader.isVisible({ timeout: 3000 }).catch(() => false)) {
      // Check initial expanded state
      await expect(swimlaneHeader).toHaveAttribute('aria-expanded', 'true');
      // Click to collapse
      await swimlaneHeader.click();
      await expect(swimlaneHeader).toHaveAttribute('aria-expanded', 'false');
      // Click to expand again
      await swimlaneHeader.click();
      await expect(swimlaneHeader).toHaveAttribute('aria-expanded', 'true');
    }
  });

  test('P1-06: card badges show phase tag', async ({ page }) => {
    await goToPlugin(page, '/board');
    // Phase badges should be visible on cards (P1, P2, P3)
    const phaseBadge = page.locator('.card-badge.phase-tag').first();
    await expect(phaseBadge).toBeVisible({ timeout: 5000 });
    const text = await phaseBadge.textContent();
    expect(text).toMatch(/P\d/);
  });

  test('P1-07: epic cards show progress bar', async ({ page }) => {
    await goToPlugin(page, '/board');
    const epicCard = page.locator('[data-type="epic"]').first();
    await expect(epicCard).toBeVisible({ timeout: 5000 });
    const progressBar = epicCard.locator('.card-progress');
    // Epic cards with stories should have a progress bar
    if (await progressBar.isVisible({ timeout: 1000 }).catch(() => false)) {
      await expect(progressBar.locator('.card-progress-fill')).toBeVisible();
    }
  });

  test('P1-08: column counts match visible cards', async ({ page }) => {
    await goToPlugin(page, '/board');
    // Check the "Done" column count matches the number of cards in it
    const doneColumns = page.locator('[data-status="done"]');
    if (await doneColumns.first().isVisible({ timeout: 3000 }).catch(() => false)) {
      const firstDoneCol = doneColumns.first();
      const countBadge = firstDoneCol.locator('.column-count');
      const countText = await countBadge.textContent();
      const expectedCount = parseInt(countText ?? '0');
      const actualCards = await firstDoneCol.locator('.board-card').count();
      expect(actualCards).toBe(expectedCount);
    }
  });
});
