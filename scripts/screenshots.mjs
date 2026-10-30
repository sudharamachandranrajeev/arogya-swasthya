// Renders the README screenshots from made-up demo data in the browser
// preview, where records live only in memory.
//
//   npm run dev                       (in another terminal)
//   node scripts/screenshots.mjs      (CHROMIUM=/path/to/chromium if not /usr/bin/chromium)
import { chromium } from 'playwright-core'

const URL = process.env.URL ?? 'http://localhost:5173/'
const OUT = new globalThis.URL('../docs/screenshots/', import.meta.url).pathname

const browser = await chromium.launch({ executablePath: process.env.CHROMIUM ?? '/usr/bin/chromium' })
const page = await browser.newPage({ viewport: { width: 1280, height: 820 }, colorScheme: 'light' })
const errors = []
page.on('pageerror', (e) => errors.push(e.message))

const click = (name) => page.getByRole('button', { name, exact: true }).click()
const shot = (name, full = false) => page.screenshot({ path: `${OUT}${name}.png`, fullPage: full })
const go = async (hash) => {
  await page.goto(`${URL}#/${hash}`)
  await page.waitForTimeout(300)
}

// First run, with two homes so the emergency line shows.
await page.goto(URL)
await click('Start')
await page.selectOption('#pl-add', 'GB')
await click('I live here')
await page.selectOption('#pl-add', 'IN')
await click('I live here')
await click('Continue')
await page.getByRole('checkbox').check()
await click('Accept and continue')
await click('Show my recovery key')
await shot('setup-recovery')
const key = (await page.locator('code.key').textContent()).trim()
await click('I have kept it safe')
const group = Number((await page.locator('label[for=grp]').textContent()).match(/group (\d+)/)[1])
await page.fill('#grp', key.split('-')[group - 1])
await click('Check')
await click('Not now')
await click('Continue without a second copy')
await page.fill('#p-name', 'Alex Demo')
await page.fill('#p-birthDate', '1980-04-02')
await click('Finish')
await page.waitForSelector('text=Lately')

// Fill a record form: plain inputs by field key, selects by value.
async function add(hash, button, values, kind) {
  await go(hash)
  if (kind) await page.getByRole('radio', { name: kind }).or(page.getByRole('button', { name: kind, exact: true })).first().click()
  await click('Add')
  for (const [k, v] of Object.entries(values)) {
    const el = page.locator(`[id$=-${k}]`).first()
    if ((await el.evaluate((e) => e.tagName)) === 'SELECT') await el.selectOption(v)
    else await el.fill(v)
  }
  await click(button)
  await page.waitForTimeout(200)
}

await add('records', 'Add condition', { name: 'Asthma', status: 'active', onset: '2015' })
await add('records', 'Add condition', { name: 'Type 2 diabetes', status: 'active', onset: '2021-03' })
await add('records', 'Add allergy', { name: 'Penicillin', reaction: 'Rash', onset: '2009' }, 'Allergies')
await add('records', 'Add vaccine', { name: 'Influenza', date: '2025-10-04', dose: '1' }, 'Vaccines')
await add('medicines', 'Add medicine', { name: 'Metformin 500 mg', dose: 'One tablet twice a day with food', status: 'active', start: '2021-03-15', reason: 'Type 2 diabetes' })
await add('medicines', 'Add medicine', { name: 'Salbutamol inhaler', dose: 'Two puffs when needed', status: 'active', start: '2015-06' })
await add('results', 'Add result', { name: 'HbA1c', value: '52', unit: 'mmol/mol', low: '20', high: '41', date: '2025-06-01' })
await add('results', 'Add result', { name: 'HbA1c', value: '47', unit: 'mmol/mol', low: '20', high: '41', date: '2025-12-02' })
await add('results', 'Add result', { name: 'Cholesterol', value: '4.6', unit: 'mmol/L', low: '0', high: '5', date: '2025-12-02' })

await page.getByRole('button', { name: 'Close' }).click()
await go('overview')
await shot('hero')
await go('results')
await shot('results')
await go('timeline')
await shot('timeline')
await go('records')
await page.getByRole('radio', { name: 'Conditions' }).or(page.getByRole('button', { name: 'Conditions', exact: true })).first().click()
await click('Add')
await page.locator('.date:has([id$=-onset]) button.open').click()
await page.waitForTimeout(200)
await shot('calendar')
await go('settings')
await page.getByRole('radio', { name: 'Places' }).or(page.getByRole('button', { name: 'Places', exact: true })).first().click()
await page.waitForTimeout(200)
await shot('places')
await go('appearance')
await shot('appearance')

await page.setViewportSize({ width: 390, height: 844 })
await go('medicines')
await shot('phone')

await browser.close()
if (errors.length) {
  console.error(errors.join('\n'))
  process.exit(1)
}
console.log(`Saved to ${OUT}`)
