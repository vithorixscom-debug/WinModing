export function instancePlaySeconds(instance) {
  return (
    Number(instance?.recent_time_played || 0) + Number(instance?.submitted_time_played || 0)
  )
}

export function formatPlaytimeParts(seconds) {
  const value = Math.max(0, Math.floor(Number(seconds) || 0))
  if (value < 60) return null
  return {
    hours: Math.floor(value / 3600),
    minutes: Math.floor((value % 3600) / 60),
  }
}

export function playtimeLabel(seconds, t) {
  const parts = formatPlaytimeParts(seconds)
  if (!parts) return t('instance.playtimeNever')
  if (parts.hours <= 0) return t('instance.playtimeMinutes', { n: parts.minutes })
  if (parts.minutes <= 0) return t('instance.playtimeHours', { n: parts.hours })
  return t('instance.playtimeHoursMinutes', { h: parts.hours, m: parts.minutes })
}
