// The terms every person accepts before using the app. Bump TERMS_VERSION
// when the meaning changes; the app then asks again.

import { emergencyLine, type Places } from './places.ts'

export const TERMS_VERSION = 1

export function terms(places: Places): { heading: string; points: string[] } {
  return {
    heading: 'Before you start',
    points: [
      'Arogya Swasthya helps you keep and understand your own health records. It is not a doctor and does not give medical advice or a diagnosis.',
      'You use it at your own risk. Check anything important with a qualified health professional before acting on it.',
      'If you choose to use AI later, its answers may list possible causes with sources. AI can be wrong; treat its answers as questions to take to a professional.',
      `In an emergency, call your local emergency number now; do not use this app. ${emergencyLine(places)}.`,
      'Your records stay encrypted on your own devices. Nothing is sent anywhere unless you choose to send it, and you are shown what is sent first.',
      'If you lose both this device and your recovery key, nobody, including us, can recover your records.',
    ],
  }
}
