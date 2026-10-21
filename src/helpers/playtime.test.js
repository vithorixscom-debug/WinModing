import { describe, expect, it } from 'vitest'
import { formatPlaytimeParts, instancePlaySeconds } from './playtime.js'

describe('instancePlaySeconds', () => {
  it('sums recent and submitted seconds', () => {
    expect(
      instancePlaySeconds({ recent_time_played: 3600, submitted_time_played: 1800 }),
    ).toBe(5400)
    expect(instancePlaySeconds({})).toBe(0)
  })
})

describe('formatPlaytimeParts', () => {
  it('treats less than a minute as empty parts', () => {
    expect(formatPlaytimeParts(0)).toBeNull()
    expect(formatPlaytimeParts(59)).toBeNull()
  })

  it('splits hours and minutes', () => {
    expect(formatPlaytimeParts(60)).toEqual({ hours: 0, minutes: 1 })
    expect(formatPlaytimeParts(3600)).toEqual({ hours: 1, minutes: 0 })
    expect(formatPlaytimeParts(5400)).toEqual({ hours: 1, minutes: 30 })
  })
})
