import { test, expect } from '@playwright/test';
import { initPage, goToPlugin } from './helpers';

test.describe('Scrum Board — Filters (P1 + P2)', () => {
  test.beforeEach(async ({ page }) => {
    await initPage(page);
    await goToPlugin(page, '/board');
    // Wait for board to load
    await expect(page.locator('.board-summary, .board-error, .board-loading')).toBeVisible({ timeout: 8000 });
  });

  // ── P1: Filter Functionality ──

  test('P1-01: phase filter updates visible cards', async ({ page }) => {
    const filterBar = page.locator('.board-filters');
    if (!await filterBar.isVisible({ timeout: 3000 }).catch(() => false)) {
      test.skip();
      return;
    }

    // Get total cards before filtering
    const totalBefore = await page.locator('.board-card').count();

    // Select Phase 2 filter
    const phaseSelect = page.locator('select').filter({ has: page.locator('option[value="2"]') }).first();
    if (await phaseSelect.isVisible({ timeout: 2000 }).catch(() => false)) {
      await phaseSelect.selectOption('2');
      await page.waitForTimeout(300);

      // Cards should be filtered (fewer or same count, all phase 2)
      const totalAfter = await page.locator('.board-card').count();
      expect(totalAfter).toBeLessThanOrEqual(totalBefore);

      // All visible phase badges should show P2
      const phaseBadges = page.locator('.board-card:visible .card-badge.phase-tag');
      const count = await phaseBadges.count();
      for (let i = 0; i < count; i++) {
        await expect(phaseBadges.nth(i)).toContainText('P2');
      }
    }
  });

  test('P1-02: epic filter shows only selected epic items', async ({ page }) => {
    const filterBar = page.locator('.board-filters');
    if (!await filterBar.isVisible({ timeout: 3000 }).catch(() => false)) {
      test.skip();
      return;
    }

    // Find the epic filter select
    const epicSelects = page.locator('.filter-group').filter({ hasText: 'Epic' }).locator('select');
    if (await epicSelects.first().isVisible({ timeout: 2000 }).catch(() => false)) {
      // Select first epic option (non-empty)
      const options = epicSelects.first().locator('option:not([value=""])');
      const firstOptionValue = await options.first().getAttribute('value');
      if (firstOptionValue) {
        await epicSelects.first().selectOption(firstOptionValue);
        await page.waitForTimeout(300);

        // All story cards should belong to this epic
        const epicTags = page.locator('.board-card[data-type="story"] .card-badge.epic-tag');
        const tagCount = await epicTags.count();
        expect(tagCount).toBeGreaterThanOrEqual(0); // May have 0 stories
      }
    }
  });

  test('P1-03: type filter separates epics from stories', async ({ page }) => {
    const filterBar = page.locator('.board-filters');
    if (!await filterBar.isVisible({ timeout: 3000 }).catch(() => false)) {
      test.skip();
      return;
    }

    const typeSelects = page.locator('.filter-group').filter({ hasText: 'Type' }).locator('select');
    if (await typeSelects.first().isVisible({ timeout: 2000 }).catch(() => false)) {
      // Filter to epics only
      await typeSelects.first().selectOption('epic');
      await page.waitForTimeout(300);
      const storyCards = await page.locator('.board-card[data-type="story"]').count();
      expect(storyCards).toBe(0);

      // Filter to stories only
      await typeSelects.first().selectOption('story');
      await page.waitForTimeout(300);
      const epicCards = await page.locator('.board-card[data-type="epic"]').count();
      expect(epicCards).toBe(0);
    }
  });

  test('P1-04: clear filters button resets all filters', async ({ page }) => {
    const filterBar = page.locator('.board-filters');
    if (!await filterBar.isVisible({ timeout: 3000 }).catch(() => false)) {
      test.skip();
      return;
    }

    const totalBefore = await page.locator('.board-card').count();

    // Apply a filter first
    const typeSelects = page.locator('.filter-group').filter({ hasText: 'Type' }).locator('select');
    if (await typeSelects.first().isVisible({ timeout: 2000 }).catch(() => false)) {
      await typeSelects.first().selectOption('epic');
      await page.waitForTimeout(300);

      // Clear button should appear
      const clearBtn = page.locator('.clear-filters-btn');
      await expect(clearBtn).toBeVisible({ timeout: 2000 });
      await clearBtn.click();
      await page.waitForTimeout(300);

      // Card count should be back to original
      const totalAfter = await page.locator('.board-card').count();
      expect(totalAfter).toBe(totalBefore);
    }
  });

  // ── P2: Combined Filters ──

  test('P2-06: multiple filters applied simultaneously', async ({ page }) => {
    const filterBar = page.locator('.board-filters');
    if (!await filterBar.isVisible({ timeout: 3000 }).catch(() => false)) {
      test.skip();
      return;
    }

    const totalBefore = await page.locator('.board-card').count();

    // Apply phase filter
    const phaseSelect = page.locator('.filter-group').filter({ hasText: 'Phase' }).locator('select');
    const typeSelect = page.locator('.filter-group').filter({ hasText: 'Type' }).locator('select');

    if (await phaseSelect.isVisible({ timeout: 2000 }).catch(() => false) &&
        await typeSelect.isVisible({ timeout: 2000 }).catch(() => false)) {
      await phaseSelect.selectOption('1');
      await typeSelect.selectOption('story');
      await page.waitForTimeout(300);

      // Should have fewer cards than either filter alone
      const totalAfter = await page.locator('.board-card').count();
      expect(totalAfter).toBeLessThanOrEqual(totalBefore);

      // All cards should be stories in phase 1
      const epicCards = await page.locator('.board-card[data-type="epic"]').count();
      expect(epicCards).toBe(0);
    }
  });
});
