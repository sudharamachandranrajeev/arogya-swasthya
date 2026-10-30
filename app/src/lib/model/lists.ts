// Short pick lists offered while typing. Plain common names only (no SNOMED,
// which cannot ship worldwide); anything else can be typed freely.

export const lists = {
  conditions: [
    'Asthma', 'Anxiety', 'Arthritis', 'Atrial fibrillation', 'Back pain', 'Chronic kidney disease', 'COPD',
    'Coronary heart disease', 'Depression', 'Eczema', 'Epilepsy', 'Gastro-oesophageal reflux', 'Gout',
    'Heart failure', 'High blood pressure', 'High cholesterol', 'Hypothyroidism', 'Hyperthyroidism',
    'Irritable bowel syndrome', 'Migraine', 'Obesity', 'Osteoporosis', 'Psoriasis', 'Sleep apnoea', 'Stroke',
    'Type 1 diabetes', 'Type 2 diabetes',
  ],
  medicines: [
    'Amlodipine', 'Atorvastatin', 'Bisoprolol', 'Cetirizine', 'Citalopram', 'Ibuprofen', 'Insulin', 'Lansoprazole',
    'Levothyroxine', 'Lisinopril', 'Losartan', 'Metformin', 'Omeprazole', 'Paracetamol', 'Ramipril',
    'Salbutamol inhaler', 'Sertraline', 'Simvastatin', 'Vitamin D', 'Warfarin',
  ],
  allergens: [
    'Penicillin', 'Amoxicillin', 'Aspirin', 'Ibuprofen', 'Codeine', 'Sulfonamides', 'Latex', 'Peanuts',
    'Tree nuts', 'Shellfish', 'Fish', 'Eggs', 'Milk', 'Wheat', 'Soy', 'Sesame', 'Pollen', 'Dust mites',
    'Cats', 'Dogs', 'Bee or wasp stings',
  ],
  reactions: ['Rash', 'Hives', 'Itching', 'Swelling', 'Wheezing', 'Vomiting', 'Diarrhoea', 'Anaphylaxis'],
  procedures: [
    'Appendectomy', 'Caesarean section', 'Cataract surgery', 'Colonoscopy', 'Coronary angioplasty',
    'Endoscopy', 'Gallbladder removal', 'Hernia repair', 'Hip replacement', 'Knee arthroscopy',
    'Knee replacement', 'Tonsillectomy',
  ],
  vaccines: [
    'COVID-19', 'Influenza', 'Hepatitis A', 'Hepatitis B', 'HPV', 'MMR', 'Pneumococcal', 'Polio',
    'Shingles', 'Tetanus, diphtheria, polio', 'Typhoid', 'Yellow fever', 'BCG (tuberculosis)', 'Meningitis ACWY',
  ],
  symptoms: [
    'Headache', 'Tiredness', 'Pain', 'Back pain', 'Joint pain', 'Chest pain', 'Shortness of breath', 'Cough',
    'Dizziness', 'Nausea', 'Low mood', 'Anxiety', 'Poor sleep', 'Fever', 'Stomach ache', 'Itching',
  ],
  tests: [
    'HbA1c', 'Fasting glucose', 'Total cholesterol', 'LDL cholesterol', 'HDL cholesterol', 'Triglycerides',
    'Haemoglobin', 'White cell count', 'Platelets', 'Ferritin', 'Vitamin D', 'Vitamin B12', 'TSH',
    'Free T4', 'Creatinine', 'eGFR', 'Potassium', 'Sodium', 'ALT', 'CRP', 'PSA',
  ],
  units: ['mmol/L', 'mmol/mol', 'mg/dL', 'g/L', 'g/dL', 'µmol/L', 'nmol/L', 'pmol/L', 'mU/L', 'IU/L', 'U/L', 'mg/L', 'ng/mL', 'µg/L', '%', '×10⁹/L'],
  bodySites: [
    'Head', 'Neck', 'Chest', 'Heart', 'Lungs', 'Abdomen', 'Lower back', 'Upper back', 'Left shoulder',
    'Right shoulder', 'Left arm', 'Right arm', 'Left hand', 'Right hand', 'Left hip', 'Right hip', 'Left knee',
    'Right knee', 'Left foot', 'Right foot', 'Skin',
  ],
  documents: [
    'Clinic letter', 'Discharge summary', 'Lab report', 'Scan or X-ray report', 'Prescription', 'Referral',
    'Vaccination card', 'Insurance document', 'Photo', 'Recording', 'Other',
  ],
} as const
