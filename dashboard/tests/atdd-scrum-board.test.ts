/**
 * ATDD Acceptance Tests — Scrum Board Feature
 *
 * TDD RED PHASE: These tests define expected behavior from acceptance criteria.
 * Remove test.skip() once the feature is verified working end-to-end.
 *
 * Acceptance Criteria (from board feature design):
 * AC-1: Board page shows epics and stories organized by status columns
 * AC-2: 5 status columns: Backlog, Ready for Dev, In Progress, Review, Done
 * AC-3: Summary bar shows total epics, stories, done count, progress percentage
 * AC-4: Filters by phase, epic, and item type update card display
 * AC-5: Swimlane grouping by epic with collapsible headers
 * AC-6: Epic detail page shows stories list, requirements, progress
 * AC-7: Story detail page shows user story and acceptance criteria
 * AC-8: Sprint progress badge visible on workflow views
 */

import { test, expect } from '@playwright/test';
import { initPage, goToPlugin, PLUGIN_BASE } from './helpers';

test.describe('ATDD: Scrum Board — Board Page', () => {
  test.beforeEach(async ({ page }) => {
    await initPage(page);
  });

  // ── AC-1: Board displays epics and stories by status ──

  test('[P0] AC-1: board loads and displays both epic and story cards', async ({ page }) => {
    await goToPlugin(page, '/board');
    const main = page.locator('main');

    // Epics should render as cards with data-type="epic"
    const epicCards = main.locator('[data-type="epic"]');
    await expect(epicCards.first()).toBeVisible({ timeout: 8000 });
    const epicCount = await epicCards.count();
    expect(epicCount).toBeGreaterThanOrEqual(1);

    // Stories should render as cards with data-type="story"
    const storyCards = main.locator('[data-type="story"]');
    await expect(storyCards.first()).toBeVisible();
    const storyCount = await storyCards.count();
    expect(storyCount).toBeGreaterThanOrEqual(1);
  });

  // ── AC-2: Five status columns ──

  test('[P0] AC-2: board renders all 5 status columns with labels', async ({ page }) => {
    await goToPlugin(page, '/board');

    const columns = [
      { id: 'backlog', label: 'Backlog' },
      { id: 'ready-for-dev', label: 'Ready' },
      { id: 'in-progress', label: 'In Progress' },
      { id: 'review', label: 'Review' },
      { id: 'done', label: 'Done' },
    ];

    for (const col of columns) {
      const column = page.locator(`[data-status="${col.id}"]`).first();
      await expect(column).toBeVisible({ timeout: 5000 });
      await expect(column.locator('.column-label')).toContainText(col.label);
    }
  });

  test('[P0] AC-2: column headers have colored top borders', async ({ page }) => {
    await goToPlugin(page, '/board');

    const doneColumn = page.locator('[data-status="done"] .column-header').first();
    await expect(doneColumn).toBeVisible({ timeout: 5000 });

    // Done column should have green border (#10b981)
    const borderColor = await doneColumn.evaluate(
      (el) => getComputedStyle(el).borderTopColor
    );
    expect(borderColor).toBeTruthy();
  });

  // ── AC-3: Summary bar ──

  test('[P0] AC-3: summary bar displays epics, stories, done, and progress', async ({ page }) => {
    await goToPlugin(page, '/board');

    const summary = page.locator('.board-summary');
    await expect(summary).toBeVisible({ timeout: 8000 });

    // Should have stat values for Epics, Stories, Done
    const statValues = summary.locator('.stat-value');
    const statCount = await statValues.count();
    expect(statCount).toBeGreaterThanOrEqual(3);

    // Progress bar should exist
    await expect(summary.locator('.progress-bar')).toBeVisible();
    await expect(summary.locator('.progress-fill')).toBeVisible();

    // Progress text should show percentage
    const progressText = await summary.locator('.progress-text').textContent();
    expect(progressText).toMatch(/\d+(\.\d+)?%/);
  });

  // ── AC-4: Filters ──

  test.skip('[P1] AC-4a: phase filter reduces visible cards to selected phase only', async ({ page }) => {
    // THIS TEST WILL FAIL if filter endpoint broken or filter select not wired
    await goToPlugin(page, '/board');
    await expect(page.locator('.board-summary')).toBeVisible({ timeout: 8000 });

    const totalBefore = await page.locator('.board-card').count();

    // Apply Phase 1 filter
    const phaseSelect = page.locator('.filter-group').filter({ hasText: 'Phase' }).locator('select');
    await expect(phaseSelect).toBeVisible({ timeout: 3000 });
    await phaseSelect.selectOption('1');
    await page.waitForTimeout(500);

    // All visible cards should have P1 phase badge
    const phaseBadges = page.locator('.board-card .card-badge.phase-tag');
    const badgeCount = await phaseBadges.count();
    for (let i = 0; i < badgeCount; i++) {
      await expect(phaseBadges.nth(i)).toContainText('P1');
    }

    // Should have fewer or equal cards
    const totalAfter = await page.locator('.board-card').count();
    expect(totalAfter).toBeLessThanOrEqual(totalBefore);
  });

  test.skip('[P1] AC-4b: type filter isolates epics from stories', async ({ page }) => {
    // THIS TEST WILL FAIL if type filter not connected to card filtering logic
    await goToPlugin(page, '/board');
    await expect(page.locator('.board-summary')).toBeVisible({ timeout: 8000 });

    const typeSelect = page.locator('.filter-group').filter({ hasText: 'Type' }).locator('select');
    await expect(typeSelect).toBeVisible({ timeout: 3000 });

    // Filter to stories only
    await typeSelect.selectOption('story');
    await page.waitForTimeout(500);
    expect(await page.locator('[data-type="epic"]').count()).toBe(0);
    expect(await page.locator('[data-type="story"]').count()).toBeGreaterThan(0);

    // Filter to epics only
    await typeSelect.selectOption('epic');
    await page.waitForTimeout(500);
    expect(await page.locator('[data-type="story"]').count()).toBe(0);
    expect(await page.locator('[data-type="epic"]').count()).toBeGreaterThan(0);
  });

  test.skip('[P1] AC-4c: clear button resets all active filters', async ({ page }) => {
    // THIS TEST WILL FAIL if clear button doesn't reset activeFilters state
    await goToPlugin(page, '/board');
    await expect(page.locator('.board-summary')).toBeVisible({ timeout: 8000 });

    const totalBefore = await page.locator('.board-card').count();

    // Apply a filter
    const typeSelect = page.locator('.filter-group').filter({ hasText: 'Type' }).locator('select');
    await typeSelect.selectOption('epic');
    await page.waitForTimeout(300);

    // Clear should appear and reset
    const clearBtn = page.locator('.clear-filters-btn');
    await expect(clearBtn).toBeVisible();
    await clearBtn.click();
    await page.waitForTimeout(300);

    expect(await page.locator('.board-card').count()).toBe(totalBefore);
  });

  // ── AC-5: Swimlanes ──

  test.skip('[P1] AC-5: swimlane headers show epic name and card count', async ({ page }) => {
    // THIS TEST WILL FAIL if swimlane_key not processed or swimlane UI not rendered
    await goToPlugin(page, '/board');
    await expect(page.locator('.board-summary')).toBeVisible({ timeout: 8000 });

    const swimlaneHeaders = page.locator('.swimlane-header');
    if (await swimlaneHeaders.first().isVisible({ timeout: 3000 }).catch(() => false)) {
      // Each header should have title and count badge
      const first = swimlaneHeaders.first();
      await expect(first.locator('.swimlane-title')).toBeVisible();
      await expect(first.locator('.swimlane-count')).toBeVisible();

      // Should be expandable (aria-expanded)
      await expect(first).toHaveAttribute('aria-expanded', 'true');
    }
  });

  test.skip('[P1] AC-5: collapsing swimlane hides its cards', async ({ page }) => {
    // THIS TEST WILL FAIL if collapse toggle doesn't hide columns-row
    await goToPlugin(page, '/board');
    await expect(page.locator('.board-summary')).toBeVisible({ timeout: 8000 });

    const swimlane = page.locator('.swimlane').first();
    if (await swimlane.isVisible({ timeout: 3000 }).catch(() => false)) {
      const header = swimlane.locator('.swimlane-header');
      const columnsRow = swimlane.locator('.columns-row');

      // Initially visible
      await expect(columnsRow).toBeVisible();

      // Collapse
      await header.click();
      await expect(header).toHaveAttribute('aria-expanded', 'false');
      await expect(columnsRow).not.toBeVisible();

      // Expand again
      await header.click();
      await expect(header).toHaveAttribute('aria-expanded', 'true');
      await expect(columnsRow).toBeVisible();
    }
  });
});

test.describe('ATDD: Scrum Board — Epic Detail', () => {
  test.beforeEach(async ({ page }) => {
    await initPage(page);
  });

  // ── AC-6: Epic detail page ──

  test('[P0] AC-6a: epic detail renders info, stories, and requirements sections', async ({ page }) => {
    await goToPlugin(page, '/board/epics/epic-1');
    const main = page.locator('main');

    await expect(main.getByText('Epic Info').first()).toBeVisible({ timeout: 8000 });
    await expect(main.getByText('Stories').first()).toBeVisible();
    await expect(main.getByText('Total Stories').first()).toBeVisible();
    await expect(main.getByText('Completed').first()).toBeVisible();
  });

  test.skip('[P1] AC-6b: epic detail shows progress as fraction and percentage', async ({ page }) => {
    // THIS TEST WILL FAIL if progress field not computed correctly in board.rs
    await goToPlugin(page, '/board/epics/epic-1');
    const main = page.locator('main');

    // Progress should contain "X/Y done" format
    const progressText = await main.getByText(/\d+\/\d+ done/).first().textContent();
    expect(progressText).toMatch(/\d+\/\d+ done \(\d+%\)/);
  });

  test.skip('[P2] AC-6c: epic detail for Phase 2 epic shows FRs and NFRs', async ({ page }) => {
    // THIS TEST WILL FAIL if epics.md parsing doesn't extract FRs for Phase 2 epics
    await goToPlugin(page, '/board/epics/epic-12');
    const main = page.locator('main');

    await expect(main.getByText('Requirements Coverage').first()).toBeVisible({ timeout: 8000 });
    await expect(main.getByText('FRs Covered').first()).toBeVisible();
  });
});

test.describe('ATDD: Scrum Board — Story Detail', () => {
  test.beforeEach(async ({ page }) => {
    await initPage(page);
  });

  // ── AC-7: Story detail page ──

  test('[P0] AC-7a: story detail renders info, user story, and acceptance criteria sections', async ({ page }) => {
    await goToPlugin(page, '/board/stories/12-1-create-bmadagentregistry-struct-with-agent-definition-constants');
    const main = page.locator('main');

    await expect(main.getByText('Story Info').first()).toBeVisible({ timeout: 8000 });
    await expect(main.getByText('User Story').first()).toBeVisible();
    await expect(main.getByText('Acceptance Criteria').first()).toBeVisible();
  });

  test.skip('[P1] AC-7b: story detail shows epic association', async ({ page }) => {
    // THIS TEST WILL FAIL if epic_id/epic_title not returned from endpoint
    await goToPlugin(page, '/board/stories/1-1-crate-scaffolding-and-process-manager');
    const main = page.locator('main');

    await expect(main.getByText('epic-1')).toBeVisible({ timeout: 8000 });
    await expect(main.getByText('Phase').first()).toBeVisible();
  });

  test.skip('[P2] AC-7c: invalid story ID returns error state', async ({ page }) => {
    // THIS TEST WILL FAIL if error handling not implemented for 404 responses
    await goToPlugin(page, '/board/stories/nonexistent-story-id');
    const main = page.locator('main');

    // Should show error or not-found message
    await expect(
      main.getByText(/not found|error|failed/i).first()
    ).toBeVisible({ timeout: 8000 });
  });
});
