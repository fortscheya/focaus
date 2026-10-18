import { expect, test } from '@playwright/test'

test('FRAME smoke page loads and the Vue control responds', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('heading', { name: '路演演示工具已就绪' })).toBeVisible()
  await page.keyboard.press('ArrowRight')
  await expect(page.getByRole('heading', { name: '工具链烟雾页' })).toBeVisible()

  const button = page.getByRole('button', { name: /彩排动作/ })
  await expect(button).toHaveText('彩排动作 0 次')
  await button.click()
  await expect(button).toHaveText('彩排动作 1 次')
  await expect(page.getByText('ECharts 已连接')).toBeVisible()
  await expect(page.locator('canvas')).toBeVisible()
  const fitsViewport = await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth + 1)
  expect(fitsViewport).toBe(true)
})
