export const IDENTITY_RECURSOS = {
  agente: 'Agentes',
  equipe: 'Equipes',
} as const

export const IDENTITY_ESTILOS = {
  personagem: 'Personagem',
  robo: 'Robô',
  criatura: 'Criatura',
  mitologico: 'Mitológico',
  geometrico: 'Geométrico',
  abstrato: 'Abstrato',
  elemento: 'Elemento',
  mineral: 'Mineral e joia',
  objeto: 'Objeto',
  pixel: 'Pixel / low-poly',
} as const

export const IDENTITY_PALETAS = {
  quente: 'Quente',
  fria: 'Fria',
  neutra: 'Neutra',
} as const

export type IdentityRecurso = keyof typeof IDENTITY_RECURSOS
export type IdentityEstilo = keyof typeof IDENTITY_ESTILOS
export type IdentityPaleta = keyof typeof IDENTITY_PALETAS

export interface IdentityConcept {
  id: string
  label: string
  recurso: IdentityRecurso
  estilo: IdentityEstilo
  paleta: IdentityPaleta
  art: string
}

export interface IdentityFamily {
  id: string
  label: string
  concepts: IdentityConcept[]
}

export const IDENTITY_FAMILIES: IdentityFamily[] = [
  {
    id: 'gotas',
    label: 'Gotas',
    concepts: [
      {
        id: 'lume',
        label: 'Lume',
        recurso: 'agente',
        estilo: 'personagem',
        paleta: 'fria',
        art: '<ellipse cx="32" cy="58" rx="14" ry="2.5" fill="#1c1c21" opacity=".12"></ellipse><path d="M32 5 C 26 16, 12 28, 12 39 a20 19 0 0 0 40 0 C 52 28, 38 16, 32 5 Z" fill="#4f7cff"></path><path d="M32 5 C 38 16, 52 28, 52 39 a20 19 0 0 1 -20 19 C 44 52, 47 40, 32 5 Z" fill="#2f56d6" opacity=".45"></path><ellipse cx="22" cy="30" rx="3" ry="5" fill="#fff" opacity=".55" transform="rotate(25 22 30)"></ellipse><circle cx="25" cy="40" r="4.2" fill="#fff"></circle><circle cx="25" cy="40" r="2.1" fill="#1c1c21"></circle><circle cx="39" cy="40" r="4.2" fill="#fff"></circle><circle cx="39" cy="40" r="2.1" fill="#1c1c21"></circle><path d="M27 48 Q32 53 37 48" fill="none" stroke="#1c1c21" stroke-width="2" stroke-linecap="round"></path>',
      },
      {
        id: 'ambar',
        label: 'Âmbar',
        recurso: 'agente',
        estilo: 'personagem',
        paleta: 'quente',
        art: '<ellipse cx="32" cy="58" rx="14" ry="2.5" fill="#1c1c21" opacity=".12"></ellipse><path d="M32 5 C 26 16, 12 28, 12 39 a20 19 0 0 0 40 0 C 52 28, 38 16, 32 5 Z" fill="#f08a3c"></path><path d="M32 5 C 38 16, 52 28, 52 39 a20 19 0 0 1 -20 19 C 44 52, 47 40, 32 5 Z" fill="#c8611c" opacity=".45"></path><ellipse cx="22" cy="30" rx="3" ry="5" fill="#fff" opacity=".55" transform="rotate(25 22 30)"></ellipse><circle cx="25" cy="40" r="4.8" fill="#fff"></circle><circle cx="26.4" cy="39" r="2.4" fill="#1c1c21"></circle><circle cx="39" cy="40" r="4.8" fill="#fff"></circle><circle cx="40.4" cy="39" r="2.4" fill="#1c1c21"></circle><circle cx="32" cy="49" r="2.2" fill="#1c1c21"></circle>',
      },
      {
        id: 'vesper',
        label: 'Vésper',
        recurso: 'agente',
        estilo: 'personagem',
        paleta: 'fria',
        art: '<ellipse cx="32" cy="58" rx="14" ry="2.5" fill="#1c1c21" opacity=".12"></ellipse><path d="M32 5 C 26 16, 12 28, 12 39 a20 19 0 0 0 40 0 C 52 28, 38 16, 32 5 Z" fill="#8b6cf0"></path><path d="M32 5 C 38 16, 52 28, 52 39 a20 19 0 0 1 -20 19 C 44 52, 47 40, 32 5 Z" fill="#6345cc" opacity=".45"></path><ellipse cx="22" cy="30" rx="3" ry="5" fill="#fff" opacity=".55" transform="rotate(25 22 30)"></ellipse><path d="M20.8 40 Q25 43.78 29.2 40" fill="none" stroke="#1c1c21" stroke-width="2" stroke-linecap="round"></path><path d="M34.8 40 Q39 43.78 43.2 40" fill="none" stroke="#1c1c21" stroke-width="2" stroke-linecap="round"></path><path d="M29 49 Q32 51 35 49" fill="none" stroke="#1c1c21" stroke-width="1.8" stroke-linecap="round"></path>',
      },
    ],
  },
  {
    id: 'cristais',
    label: 'Cristais',
    concepts: [
      {
        id: 'quarzo',
        label: 'Quarzo',
        recurso: 'agente',
        estilo: 'geometrico',
        paleta: 'fria',
        art: '<polygon points="32,4 54,17 54,45 32,58 10,45 10,17" fill="#2bb3a8"></polygon><polygon points="32,4 54,17 32,30 10,17" fill="#fff" opacity=".35"></polygon><polygon points="32,30 54,17 54,45 32,58" fill="#1a857c" opacity=".55"></polygon><path d="M32 30 V58M10 17 L32 30 L54 17" stroke="#fff" stroke-opacity=".5" stroke-width="1.2" fill="none"></path><circle cx="25" cy="36" r="3.2" fill="#fff"></circle><circle cx="25" cy="36" r="1.6" fill="#1c1c21"></circle><circle cx="39" cy="36" r="3.2" fill="#fff"></circle><circle cx="39" cy="36" r="1.6" fill="#1c1c21"></circle>',
      },
      {
        id: 'onix',
        label: 'Ônix',
        recurso: 'agente',
        estilo: 'geometrico',
        paleta: 'fria',
        art: '<polygon points="32,3 56,30 32,61 8,30" fill="#8b6cf0"></polygon><polygon points="32,3 56,30 32,30" fill="#6345cc" opacity=".6"></polygon><polygon points="8,30 32,30 32,61" fill="#fff" opacity=".25"></polygon><path d="M8 30 H56M32 3 V61" stroke="#fff" stroke-opacity=".45" stroke-width="1.2"></path><circle cx="26" cy="38" r="3" fill="#fff"></circle><circle cx="26" cy="38" r="1.5" fill="#1c1c21"></circle><circle cx="38" cy="38" r="3" fill="#fff"></circle><circle cx="38" cy="38" r="1.5" fill="#1c1c21"></circle>',
      },
      {
        id: 'selene',
        label: 'Selene',
        recurso: 'agente',
        estilo: 'geometrico',
        paleta: 'quente',
        art: '<polygon points="32,2 46,22 40,62 24,62 18,22" fill="#e0609a"></polygon><polygon points="32,2 46,22 32,26 18,22" fill="#fff" opacity=".4"></polygon><polygon points="32,26 46,22 40,62 32,62" fill="#b83d74" opacity=".55"></polygon><path d="M32 26 V62" stroke="#fff" stroke-opacity=".5" stroke-width="1.2"></path><circle cx="28" cy="36" r="2.8" fill="#fff"></circle><circle cx="28" cy="36" r="1.4" fill="#1c1c21"></circle><circle cx="37" cy="36" r="2.8" fill="#fff"></circle><circle cx="37" cy="36" r="1.4" fill="#1c1c21"></circle>',
      },
    ],
  },
  {
    id: 'automatos',
    label: 'Autômatos',
    concepts: [
      {
        id: 'ferro',
        label: 'Ferro-9',
        recurso: 'agente',
        estilo: 'robo',
        paleta: 'neutra',
        art: '<path d="M32 4 V12" stroke="#3e4556" stroke-width="2.5" stroke-linecap="round"></path><circle cx="32" cy="4" r="3" fill="#e0524a"></circle><rect x="12" y="12" width="40" height="34" rx="6" fill="#5c6478"></rect><rect x="8" y="22" width="4" height="12" rx="2" fill="#3e4556"></rect><rect x="52" y="22" width="4" height="12" rx="2" fill="#3e4556"></rect><rect x="18" y="20" width="10" height="9" rx="2" fill="#bfe6ff"></rect><rect x="36" y="20" width="10" height="9" rx="2" fill="#bfe6ff"></rect><rect x="21" y="23" width="4" height="4" fill="#1c1c21"></rect><rect x="39" y="23" width="4" height="4" fill="#1c1c21"></rect><path d="M22 37 H42M26 34 V40M32 34 V40M38 34 V40" stroke="#3e4556" stroke-width="2"></path><rect x="20" y="48" width="24" height="10" rx="3" fill="#3e4556"></rect>',
      },
      {
        id: 'cobre',
        label: 'Bronzo',
        recurso: 'agente',
        estilo: 'robo',
        paleta: 'quente',
        art: '<circle cx="32" cy="30" r="22" fill="#c9733c"></circle><circle cx="32" cy="30" r="22" fill="none" stroke="#9a5023" stroke-width="2"></circle><rect x="14" y="22" width="36" height="12" rx="6" fill="#2b2f4a"></rect><circle cx="24" cy="28" r="3" fill="#7ef0d4"></circle><circle cx="40" cy="28" r="3" fill="#7ef0d4"></circle><circle cx="10" cy="30" r="3" fill="#9a5023"></circle><circle cx="54" cy="30" r="3" fill="#9a5023"></circle><path d="M26 42 Q32 45 38 42" stroke="#9a5023" stroke-width="2" fill="none" stroke-linecap="round"></path><rect x="24" y="52" width="16" height="8" rx="3" fill="#9a5023"></rect><ellipse cx="22" cy="14" rx="5" ry="2.5" fill="#fff" opacity=".35"></ellipse>',
      },
      {
        id: 'ceramica',
        label: 'Kaolin',
        recurso: 'agente',
        estilo: 'robo',
        paleta: 'neutra',
        art: '<rect x="18" y="4" width="28" height="54" rx="14" fill="#e9e2d3"></rect><rect x="18" y="4" width="28" height="54" rx="14" fill="none" stroke="#c4baa5" stroke-width="2"></rect><circle cx="32" cy="22" r="8" fill="#2b2f4a"></circle><circle cx="32" cy="22" r="4" fill="#4f7cff"></circle><circle cx="34" cy="20" r="1.4" fill="#fff"></circle><ellipse cx="24" cy="34" rx="3" ry="1.8" fill="#e0609a" opacity=".5"></ellipse><ellipse cx="40" cy="34" rx="3" ry="1.8" fill="#e0609a" opacity=".5"></ellipse><path d="M24 44 H40" stroke="#c4baa5" stroke-width="2" stroke-linecap="round"></path><path d="M24 49 H40" stroke="#c4baa5" stroke-width="2" stroke-linecap="round"></path>',
      },
    ],
  },
  {
    id: 'mascaras',
    label: 'Máscaras',
    concepts: [
      {
        id: 'anpu',
        label: 'Anpu',
        recurso: 'agente',
        estilo: 'mitologico',
        paleta: 'fria',
        art: '<path d="M14 4 L24 22 H40 L50 4 L46 26 C 46 42, 40 56, 32 60 C 24 56, 18 42, 18 26 Z" fill="#2b2f4a"></path><path d="M16 9 L22 21M48 9 L42 21" stroke="#e8b931" stroke-width="2"></path><path d="M22 32 L29 30 L27 35 Z" fill="#e8b931"></path><path d="M42 32 L35 30 L37 35 Z" fill="#e8b931"></path><path d="M28 46 H36 L32 54 Z" fill="#1a1d33" stroke="#e8b931" stroke-width="1.5"></path><path d="M24 40 H40" stroke="#e8b931" stroke-width="1.5" opacity=".6"></path>',
      },
      {
        id: 'talia',
        label: 'Talia',
        recurso: 'agente',
        estilo: 'mitologico',
        paleta: 'neutra',
        art: '<path d="M8 22 C 8 10, 56 10, 56 22 C 56 44, 44 58, 32 58 C 20 58, 8 44, 8 22 Z" fill="#f6f1e6"></path><path d="M8 22 C 8 10, 56 10, 56 22" fill="none" stroke="#c4baa5" stroke-width="2"></path><path d="M16 28 Q22 22 28 28 Q22 31 16 28Z M36 28 Q42 22 48 28 Q42 31 36 28Z" fill="#1c1c21"></path><path d="M18 40 Q32 54 46 40 Q32 46 18 40 Z" fill="#1c1c21"></path><path d="M8 20 C 2 22, 2 30, 6 34" stroke="#e0524a" stroke-width="3" fill="none" stroke-linecap="round"></path>',
      },
      {
        id: 'kemet',
        label: 'Seshat-2',
        recurso: 'agente',
        estilo: 'mitologico',
        paleta: 'quente',
        art: '<path d="M10 20 C 10 6, 54 6, 54 20 L58 50 H42 L40 34 H24 L22 50 H6 Z" fill="#4f7cff"></path><path d="M14 16 L8 48M20 12 L16 48M44 12 L48 48M50 16 L56 48" stroke="#e8b931" stroke-width="2.5"></path><rect x="22" y="18" width="20" height="30" rx="9" fill="#d9a066"></rect><circle cx="27" cy="30" r="2.6" fill="#fff"></circle><circle cx="27" cy="30" r="1.3" fill="#1c1c21"></circle><circle cx="37" cy="30" r="2.6" fill="#fff"></circle><circle cx="37" cy="30" r="1.3" fill="#1c1c21"></circle><path d="M28 40 H36" stroke="#1c1c21" stroke-width="2" stroke-linecap="round"></path><path d="M30 48 V58 H34 V48" fill="#e8b931"></path>',
      },
    ],
  },
  {
    id: 'espiritos',
    label: 'Espíritos',
    concepts: [
      {
        id: 'argos',
        label: 'Argos',
        recurso: 'agente',
        estilo: 'criatura',
        paleta: 'fria',
        art: '<ellipse cx="32" cy="58" rx="12" ry="2.5" fill="#1c1c21" opacity=".12"></ellipse><path d="M6 30 Q32 6 58 30 Q32 54 6 30 Z" fill="#fff" stroke="#1c1c21" stroke-width="2.5"></path><circle cx="32" cy="30" r="11" fill="#2bb3a8"></circle><circle cx="32" cy="30" r="5" fill="#1c1c21"></circle><circle cx="35" cy="27" r="2" fill="#fff"></circle><path d="M14 18 L10 12M24 13 L22 6M40 13 L42 6M50 18 L54 12" stroke="#1c1c21" stroke-width="2.5" stroke-linecap="round"></path>',
      },
      {
        id: 'nebula',
        label: 'Nébula',
        recurso: 'agente',
        estilo: 'criatura',
        paleta: 'fria',
        art: '<path d="M14 30 C 14 14, 50 14, 50 30 V48 L44 44 L38 52 L32 44 L26 52 L20 44 L14 48 Z" fill="#8b6cf0"></path><path d="M50 30 V48 L44 44 L38 52 L35 47 C 46 40, 46 22, 36 15 C 44 16, 50 22, 50 30Z" fill="#6345cc" opacity=".5"></path><circle cx="25" cy="30" r="3.6" fill="#fff"></circle><circle cx="25" cy="31" r="1.8" fill="#1c1c21"></circle><circle cx="39" cy="30" r="3.6" fill="#fff"></circle><circle cx="39" cy="31" r="1.8" fill="#1c1c21"></circle><circle cx="9" cy="16" r="2" fill="#8b6cf0" opacity=".6"></circle><circle cx="56" cy="10" r="1.5" fill="#8b6cf0" opacity=".6"></circle>',
      },
      {
        id: 'gruk',
        label: 'Gruk',
        recurso: 'agente',
        estilo: 'criatura',
        paleta: 'quente',
        art: '<path d="M16 20 L10 6 L24 16 Z M48 20 L54 6 L40 16 Z" fill="#e9e2d3"></path><path d="M8 38 C 8 18, 56 18, 56 38 C 56 52, 46 58, 32 58 C 18 58, 8 52, 8 38 Z" fill="#3fae6a"></path><circle cx="32" cy="32" r="9" fill="#fff"></circle><circle cx="33" cy="33" r="4.5" fill="#1c1c21"></circle><path d="M20 46 Q32 54 44 46" stroke="#1c1c21" stroke-width="2.5" fill="none" stroke-linecap="round"></path><path d="M28 48 L30 53 L32 48.6" fill="#fff"></path><circle cx="16" cy="44" r="2" fill="#24814a"></circle><circle cx="48" cy="40" r="2.5" fill="#24814a"></circle>',
      },
      {
        id: 'zeeb',
        label: 'Zeeb',
        recurso: 'agente',
        estilo: 'criatura',
        paleta: 'fria',
        art: '<path d="M22 14 L16 4M42 14 L48 4" stroke="#1a857c" stroke-width="2" stroke-linecap="round"></path><circle cx="16" cy="4" r="3" fill="#e8b931"></circle><circle cx="48" cy="4" r="3" fill="#e8b931"></circle><path d="M32 12 C 50 12, 56 26, 52 38 C 48 50, 40 58, 32 58 C 24 58, 16 50, 12 38 C 8 26, 14 12, 32 12 Z" fill="#2bb3a8"></path><ellipse cx="22" cy="34" rx="6" ry="8" fill="#1c1c21" transform="rotate(-20 22 34)"></ellipse><ellipse cx="42" cy="34" rx="6" ry="8" fill="#1c1c21" transform="rotate(20 42 34)"></ellipse><circle cx="24" cy="31" r="2" fill="#fff"></circle><circle cx="44" cy="31" r="2" fill="#fff"></circle><path d="M29 48 H35" stroke="#1c1c21" stroke-width="2" stroke-linecap="round"></path>',
      },
    ],
  },
  {
    id: 'formas',
    label: 'Formas',
    concepts: [
      {
        id: 'orbe',
        label: 'Orbe',
        recurso: 'agente',
        estilo: 'abstrato',
        paleta: 'fria',
        art: '<circle cx="32" cy="32" r="18" fill="#4f7cff"></circle><path d="M32 14 A18 18 0 0 1 32 50 A12 18 0 0 0 32 14Z" fill="#2f56d6" opacity=".5"></path><ellipse cx="32" cy="32" rx="29" ry="9" fill="none" stroke="#e8b931" stroke-width="3" transform="rotate(-18 32 32)"></ellipse><path d="M5 37 A29 9 0 0 0 59 27" fill="none" stroke="#4f7cff" stroke-width="0"></path><circle cx="25" cy="25" r="4" fill="#fff" opacity=".45"></circle>',
      },
      {
        id: 'aster',
        label: 'Áster',
        recurso: 'agente',
        estilo: 'abstrato',
        paleta: 'fria',
        art: '<polygon points="32,6 56,19 32,32 8,19" fill="#8b6cf0"></polygon><polygon points="8,19 32,32 32,58 8,45" fill="#6345cc"></polygon><polygon points="56,19 32,32 32,58 56,45" fill="#6345cc" opacity=".6"></polygon><polygon points="32,14 42,19.5 32,25 22,19.5" fill="#fff" opacity=".35"></polygon>',
      },
      {
        id: 'trina',
        label: 'Íris',
        recurso: 'agente',
        estilo: 'abstrato',
        paleta: 'quente',
        art: '<path d="M2 36 L24 33" stroke="#fff" stroke-width="3" stroke-linecap="round"></path><path d="M2 36 L24 33" stroke="var(--color-ink)" stroke-width="1" opacity=".4"></path><polygon points="32,8 54,48 10,48" fill="#f08a3c" opacity=".9"></polygon><polygon points="32,8 54,48 32,48" fill="#c8611c" opacity=".6"></polygon><polygon points="32,16 46,42 18,42" fill="#fff" opacity=".18"></polygon><path d="M40 32 L62 26" stroke="#e0524a" stroke-width="2.6" stroke-linecap="round"></path><path d="M40 32 L62 31" stroke="#e8b931" stroke-width="2.6" stroke-linecap="round"></path><path d="M40 32 L62 36" stroke="#3fae6a" stroke-width="2.6" stroke-linecap="round"></path><path d="M40 32 L62 41" stroke="#4f7cff" stroke-width="2.6" stroke-linecap="round"></path><circle cx="27" cy="36" r="2.6" fill="#fff"></circle><circle cx="27" cy="36" r="1.3" fill="#1c1c21"></circle><circle cx="35" cy="36" r="2.6" fill="#fff"></circle><circle cx="35" cy="36" r="1.3" fill="#1c1c21"></circle>',
      },
      {
        id: 'fractal',
        label: 'Sierpa',
        recurso: 'agente',
        estilo: 'abstrato',
        paleta: 'fria',
        art: '<polygon points="4,56 11.0,56 7.5,49.938" fill="#1a857c"></polygon><polygon points="11.0,56 18.0,56 14.5,49.938" fill="#1a857c"></polygon><polygon points="7.5,49.938 14.5,49.938 11.0,43.876000000000005" fill="#2bb3a8"></polygon><polygon points="18.0,56 25.0,56 21.5,49.938" fill="#1a857c"></polygon><polygon points="25.0,56 32.0,56 28.5,49.938" fill="#2bb3a8"></polygon><polygon points="21.5,49.938 28.5,49.938 25.0,43.876000000000005" fill="#2bb3a8"></polygon><polygon points="11.0,43.876 18.0,43.876 14.5,37.814" fill="#1a857c"></polygon><polygon points="18.0,43.876 25.0,43.876 21.5,37.814" fill="#1a857c"></polygon><polygon points="14.5,37.814 21.5,37.814 18.0,31.752" fill="#2bb3a8"></polygon><polygon points="32.0,56 39.0,56 35.5,49.938" fill="#2bb3a8"></polygon><polygon points="39.0,56 46.0,56 42.5,49.938" fill="#2bb3a8"></polygon><polygon points="35.5,49.938 42.5,49.938 39.0,43.876000000000005" fill="#1a857c"></polygon><polygon points="46.0,56 53.0,56 49.5,49.938" fill="#1a857c"></polygon><polygon points="53.0,56 60.0,56 56.5,49.938" fill="#1a857c"></polygon><polygon points="49.5,49.938 56.5,49.938 53.0,43.876000000000005" fill="#2bb3a8"></polygon><polygon points="39.0,43.876 46.0,43.876 42.5,37.814" fill="#2bb3a8"></polygon><polygon points="46.0,43.876 53.0,43.876 49.5,37.814" fill="#2bb3a8"></polygon><polygon points="42.5,37.814 49.5,37.814 46.0,31.752" fill="#1a857c"></polygon><polygon points="18.0,31.752 25.0,31.752 21.5,25.689999999999998" fill="#1a857c"></polygon><polygon points="25.0,31.752 32.0,31.752 28.5,25.689999999999998" fill="#1a857c"></polygon><polygon points="21.5,25.689999999999998 28.5,25.689999999999998 25.0,19.627999999999997" fill="#2bb3a8"></polygon><polygon points="32.0,31.752 39.0,31.752 35.5,25.689999999999998" fill="#2bb3a8"></polygon><polygon points="39.0,31.752 46.0,31.752 42.5,25.689999999999998" fill="#2bb3a8"></polygon><polygon points="35.5,25.689999999999998 42.5,25.689999999999998 39.0,19.627999999999997" fill="#1a857c"></polygon><polygon points="25.0,19.628 32.0,19.628 28.5,13.565999999999999" fill="#1a857c"></polygon><polygon points="32.0,19.628 39.0,19.628 35.5,13.565999999999999" fill="#2bb3a8"></polygon><polygon points="28.5,13.565999999999999 35.5,13.565999999999999 32.0,7.503999999999999" fill="#1a857c"></polygon>',
      },
    ],
  },
  {
    id: 'elementos',
    label: 'Elementos',
    concepts: [
      {
        id: 'ignis',
        label: 'Ignis',
        recurso: 'agente',
        estilo: 'elemento',
        paleta: 'quente',
        art: '<path d="M32 4 C 36 16, 52 22, 50 40 C 49 52, 40 60, 32 60 C 22 60, 14 52, 14 40 C 14 30, 22 26, 24 16 C 28 22, 30 24, 32 4 Z" fill="#f08a3c"></path><path d="M32 26 C 36 34, 42 38, 41 46 C 40 53, 36 56, 32 56 C 27 56, 23 52, 23 46 C 23 40, 28 38, 32 26 Z" fill="#e8b931"></path><circle cx="28" cy="46" r="2.4" fill="#fff"></circle><circle cx="28" cy="46" r="1.2" fill="#1c1c21"></circle><circle cx="36" cy="46" r="2.4" fill="#fff"></circle><circle cx="36" cy="46" r="1.2" fill="#1c1c21"></circle>',
      },
      {
        id: 'nimbo',
        label: 'Nimbo',
        recurso: 'agente',
        estilo: 'elemento',
        paleta: 'fria',
        art: '<path d="M16 46 C 6 46, 6 32, 16 32 C 16 20, 32 16, 36 26 C 42 16, 58 22, 54 34 C 62 36, 60 46, 52 46 Z" fill="#dfe6f3"></path><path d="M16 46 H52 C 58 46, 60 40, 56 36" fill="none" stroke="#b7c3da" stroke-width="2.5"></path><circle cx="28" cy="36" r="2.8" fill="#fff"></circle><circle cx="28" cy="37" r="1.4" fill="#1c1c21"></circle><circle cx="40" cy="36" r="2.8" fill="#fff"></circle><circle cx="40" cy="37" r="1.4" fill="#1c1c21"></circle><path d="M22 52 L20 58M32 52 L30 60M42 52 L40 58" stroke="#4f7cff" stroke-width="2.5" stroke-linecap="round"></path>',
      },
      {
        id: 'volta',
        label: 'Volta',
        recurso: 'agente',
        estilo: 'elemento',
        paleta: 'quente',
        art: '<circle cx="32" cy="32" r="26" fill="#2b2f4a"></circle><circle cx="32" cy="32" r="26" fill="none" stroke="#e8b931" stroke-width="2" stroke-dasharray="3 5"></circle><path d="M36 8 L18 36 H30 L26 56 L46 26 H34 Z" fill="#e8b931"></path><path d="M36 8 L30 26 H34 L46 26 Z" fill="#fff" opacity=".4"></path>',
      },
      {
        id: 'mercurio',
        label: 'Hidrargo',
        recurso: 'agente',
        estilo: 'elemento',
        paleta: 'neutra',
        art: '<path d="M10 40 C 6 26, 20 10, 34 12 C 50 14, 60 28, 54 42 C 50 52, 40 58, 28 56 C 18 55, 12 48, 10 40 Z" fill="#c9cfda"></path><path d="M14 38 C 14 26, 24 18, 34 18 C 22 22, 18 32, 22 44 C 18 44, 15 42, 14 38Z" fill="#fff" opacity=".8"></path><path d="M36 46 C 44 44, 50 38, 50 30" stroke="#8a93a6" stroke-width="2.5" fill="none" stroke-linecap="round"></path><circle cx="48" cy="54" r="4" fill="#c9cfda"></circle><circle cx="56" cy="50" r="2" fill="#c9cfda"></circle>',
      },
    ],
  },
  {
    id: 'objetos-vivos',
    label: 'Objetos vivos',
    concepts: [
      {
        id: 'codex',
        label: 'Codex',
        recurso: 'agente',
        estilo: 'objeto',
        paleta: 'quente',
        art: '<rect x="12" y="6" width="40" height="52" rx="4" fill="#e0524a"></rect><rect x="12" y="6" width="7" height="52" rx="2" fill="#b0332c"></rect><rect x="18" y="52" width="34" height="6" fill="#f6f1e6"></rect><path d="M18 55 H52" stroke="#c4baa5" stroke-width="1"></path><circle cx="29" cy="26" r="3.4" fill="#fff"></circle><circle cx="29" cy="27" r="1.7" fill="#1c1c21"></circle><circle cx="42" cy="26" r="3.4" fill="#fff"></circle><circle cx="42" cy="27" r="1.7" fill="#1c1c21"></circle><path d="M30 36 Q35.5 40 41 36" stroke="#1c1c21" stroke-width="2" fill="none" stroke-linecap="round"></path><path d="M26 14 H46" stroke="#e8b931" stroke-width="2"></path>',
      },
      {
        id: 'cronos',
        label: 'Horae',
        recurso: 'agente',
        estilo: 'objeto',
        paleta: 'quente',
        art: '<path d="M14 10 L22 16M50 10 L42 16" stroke="#b88c12" stroke-width="4" stroke-linecap="round"></path><circle cx="32" cy="34" r="24" fill="#e8b931"></circle><circle cx="32" cy="34" r="18" fill="#fffaf0"></circle><circle cx="47.0" cy="34.0" r="1.2" fill="#b88c12"></circle><circle cx="45.0" cy="41.5" r="1.2" fill="#b88c12"></circle><circle cx="39.5" cy="47.0" r="1.2" fill="#b88c12"></circle><circle cx="32.0" cy="49.0" r="1.2" fill="#b88c12"></circle><circle cx="24.5" cy="47.0" r="1.2" fill="#b88c12"></circle><circle cx="19.0" cy="41.5" r="1.2" fill="#b88c12"></circle><circle cx="17.0" cy="34.0" r="1.2" fill="#b88c12"></circle><circle cx="19.0" cy="26.5" r="1.2" fill="#b88c12"></circle><circle cx="24.5" cy="21.0" r="1.2" fill="#b88c12"></circle><circle cx="32.0" cy="19.0" r="1.2" fill="#b88c12"></circle><circle cx="39.5" cy="21.0" r="1.2" fill="#b88c12"></circle><circle cx="45.0" cy="26.5" r="1.2" fill="#b88c12"></circle><path d="M32 34 L24 26M32 34 L42 28" stroke="#1c1c21" stroke-width="2.5" stroke-linecap="round"></path><circle cx="32" cy="34" r="2.5" fill="#e0524a"></circle>',
      },
      {
        id: 'rumo',
        label: 'Rumo',
        recurso: 'agente',
        estilo: 'objeto',
        paleta: 'fria',
        art: '<circle cx="32" cy="32" r="27" fill="#2b2f4a"></circle><circle cx="32" cy="32" r="22" fill="none" stroke="#e9e2d3" stroke-width="1.2" opacity=".6"></circle><polygon points="32,8 37,32 32,56 27,32" fill="#e9e2d3"></polygon><polygon points="32,8 37,32 27,32" fill="#e0524a"></polygon><polygon points="8,32 32,27 56,32 32,37" fill="#c4baa5" opacity=".7"></polygon><circle cx="32" cy="32" r="3" fill="#e8b931"></circle>',
      },
      {
        id: 'clavis',
        label: 'Clavis',
        recurso: 'agente',
        estilo: 'objeto',
        paleta: 'quente',
        art: '<circle cx="22" cy="22" r="16" fill="#e8b931"></circle><circle cx="22" cy="22" r="7" fill="var(--color-surface)"></circle><path d="M33 33 L56 56 M46 46 L52 40 M51 51 L56 46" stroke="#e8b931" stroke-width="7" stroke-linecap="round"></path><circle cx="15" cy="14" r="2.4" fill="#fff"></circle><circle cx="15" cy="14" r="1.2" fill="#1c1c21"></circle><circle cx="29" cy="14" r="2.4" fill="#fff"></circle><circle cx="29" cy="14" r="1.2" fill="#1c1c21"></circle>',
      },
    ],
  },
  {
    id: 'pixel-low-poly',
    label: 'Pixel e low-poly',
    concepts: [
      {
        id: 'bit',
        label: 'Bit',
        recurso: 'agente',
        estilo: 'pixel',
        paleta: 'fria',
        art: '<rect x="18" y="4" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="25" y="4" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="32" y="4" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="39" y="4" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="11" y="11" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="18" y="11" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="25" y="11" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="32" y="11" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="39" y="11" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="46" y="11" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="4" y="18" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="11" y="18" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="18" y="18" width="7.2" height="7.2" fill="#fff"></rect><rect x="25" y="18" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="32" y="18" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="39" y="18" width="7.2" height="7.2" fill="#fff"></rect><rect x="46" y="18" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="53" y="18" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="4" y="25" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="11" y="25" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="18" y="25" width="7.2" height="7.2" fill="#1c1c21"></rect><rect x="25" y="25" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="32" y="25" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="39" y="25" width="7.2" height="7.2" fill="#1c1c21"></rect><rect x="46" y="25" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="53" y="25" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="4" y="32" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="11" y="32" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="18" y="32" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="25" y="32" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="32" y="32" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="39" y="32" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="46" y="32" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="53" y="32" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="4" y="39" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="11" y="39" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="18" y="39" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="39" y="39" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="46" y="39" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="53" y="39" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="11" y="46" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="18" y="46" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="25" y="46" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="32" y="46" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="39" y="46" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="46" y="46" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="4" y="53" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="18" y="53" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="39" y="53" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="53" y="53" width="7.2" height="7.2" fill="#3fae6a"></rect><rect x="4" y="4" width="56" height="7" fill="#fff" opacity=".0"></rect>',
      },
      {
        id: 'facet',
        label: 'Vulpa',
        recurso: 'agente',
        estilo: 'pixel',
        paleta: 'quente',
        art: '<polygon points="10,10 26,22 22,34" fill="#c8611c"></polygon><polygon points="54,10 38,22 42,34" fill="#c8611c"></polygon><polygon points="26,22 38,22 32,46" fill="#f08a3c"></polygon><polygon points="22,34 26,22 32,46" fill="#f08a3c" opacity=".8"></polygon><polygon points="42,34 38,22 32,46" fill="#c8611c" opacity=".85"></polygon><polygon points="22,34 32,46 32,58 16,44" fill="#fff" opacity=".9"></polygon><polygon points="42,34 32,46 32,58 48,44" fill="#e9e2d3"></polygon><polygon points="29,52 35,52 32,57" fill="#1c1c21"></polygon><polygon points="25,32 29,33 27,35" fill="#1c1c21"></polygon><polygon points="39,32 35,33 37,35" fill="#1c1c21"></polygon>',
      },
    ],
  },
  {
    id: 'minerais-e-joias',
    label: 'Minerais e joias',
    concepts: [
      {
        id: 'rubra',
        label: 'Rubra',
        recurso: 'agente',
        estilo: 'mineral',
        paleta: 'quente',
        art: '<polygon points="57.9,42.7 42.7,57.9 21.3,57.9 6.1,42.7 6.1,21.3 21.3,6.1 42.7,6.1 57.9,21.3" fill="#d8344f"></polygon><polygon points="57.9,42.7 42.7,57.9 37.4,44.9 44.9,37.4" fill="#fff" opacity=".18"></polygon><polygon points="42.7,57.9 21.3,57.9 26.6,44.9 37.4,44.9" fill="#a11d36" opacity=".55"></polygon><polygon points="21.3,57.9 6.1,42.7 19.1,37.4 26.6,44.9" fill="#fff" opacity=".18"></polygon><polygon points="6.1,42.7 6.1,21.3 19.1,26.6 19.1,37.4" fill="#a11d36" opacity=".55"></polygon><polygon points="6.1,21.3 21.3,6.1 26.6,19.1 19.1,26.6" fill="#fff" opacity=".18"></polygon><polygon points="21.3,6.1 42.7,6.1 37.4,19.1 26.6,19.1" fill="#a11d36" opacity=".55"></polygon><polygon points="42.7,6.1 57.9,21.3 44.9,26.6 37.4,19.1" fill="#fff" opacity=".18"></polygon><polygon points="57.9,21.3 57.9,42.7 44.9,37.4 44.9,26.6" fill="#a11d36" opacity=".55"></polygon><polygon points="44.9,37.4 37.4,44.9 26.6,44.9 19.1,37.4 19.1,26.6 26.6,19.1 37.4,19.1 44.9,26.6" fill="#d8344f" stroke="#fff" stroke-opacity=".5" stroke-width="1"></polygon><path d="M20 22 L25 26" stroke="#fff" stroke-width="2.5" stroke-linecap="round" opacity=".8"></path><circle cx="27" cy="33" r="2.6" fill="#fff"></circle><circle cx="27" cy="33" r="1.3" fill="#1c1c21"></circle><circle cx="37" cy="33" r="2.6" fill="#fff"></circle><circle cx="37" cy="33" r="1.3" fill="#1c1c21"></circle>',
      },
      {
        id: 'verdea',
        label: 'Verdea',
        recurso: 'agente',
        estilo: 'mineral',
        paleta: 'fria',
        art: '<polygon points="16.0,6.0 48.0,6.0 58.0,16.0 58.0,48.0 48.0,58.0 16.0,58.0 6.0,48.0 6.0,16.0" fill="#127657"></polygon><polygon points="20.0,13.0 44.0,13.0 51.0,20.0 51.0,44.0 44.0,51.0 20.0,51.0 13.0,44.0 13.0,20.0" fill="#1fa37a"></polygon><polygon points="24.0,20.0 40.0,20.0 44.0,24.0 44.0,40.0 40.0,44.0 24.0,44.0 20.0,40.0 20.0,24.0" fill="#1fa37a" stroke="#fff" stroke-opacity=".45" stroke-width="1"></polygon><polygon points="16.0,6.0 48.0,6.0 44.0,13.0 20.0,13.0" fill="#fff" opacity=".22"></polygon><polygon points="6.0,48.0 6.0,16.0 13.0,20.0 13.0,44.0" fill="#fff" opacity=".12"></polygon><circle cx="27" cy="33" r="2.6" fill="#fff"></circle><circle cx="27" cy="33" r="1.3" fill="#1c1c21"></circle><circle cx="37" cy="33" r="2.6" fill="#fff"></circle><circle cx="37" cy="33" r="1.3" fill="#1c1c21"></circle>',
      },
      {
        id: 'safiro',
        label: 'Safiro',
        recurso: 'agente',
        estilo: 'mineral',
        paleta: 'fria',
        art: '<ellipse cx="32" cy="32" rx="22" ry="28" fill="#2f5fd8"></ellipse><path d="M32 32 L54.0 32.0" stroke="#fff" stroke-opacity=".28" stroke-width="1.2"></path><path d="M32 32 L49.8 48.5" stroke="#fff" stroke-opacity=".28" stroke-width="1.2"></path><path d="M32 32 L38.8 58.6" stroke="#fff" stroke-opacity=".28" stroke-width="1.2"></path><path d="M32 32 L25.2 58.6" stroke="#fff" stroke-opacity=".28" stroke-width="1.2"></path><path d="M32 32 L14.2 48.5" stroke="#fff" stroke-opacity=".28" stroke-width="1.2"></path><path d="M32 32 L10.0 32.0" stroke="#fff" stroke-opacity=".28" stroke-width="1.2"></path><path d="M32 32 L14.2 15.5" stroke="#fff" stroke-opacity=".28" stroke-width="1.2"></path><path d="M32 32 L25.2 5.4" stroke="#fff" stroke-opacity=".28" stroke-width="1.2"></path><path d="M32 32 L38.8 5.4" stroke="#fff" stroke-opacity=".28" stroke-width="1.2"></path><path d="M32 32 L49.8 15.5" stroke="#fff" stroke-opacity=".28" stroke-width="1.2"></path><ellipse cx="32" cy="32" rx="11" ry="14" fill="#2f5fd8" stroke="#fff" stroke-opacity=".5" stroke-width="1"></ellipse><path d="M32 4 A22 28 0 0 1 54 32 L43 32 A11 14 0 0 0 32 18 Z" fill="#1c3e9e" opacity=".45"></path><ellipse cx="22" cy="18" rx="3" ry="5" fill="#fff" opacity=".55" transform="rotate(30 22 18)"></ellipse><circle cx="27" cy="33" r="2.6" fill="#fff"></circle><circle cx="27" cy="33" r="1.3" fill="#1c1c21"></circle><circle cx="37" cy="33" r="2.6" fill="#fff"></circle><circle cx="37" cy="33" r="1.3" fill="#1c1c21"></circle>',
      },
      {
        id: 'topaz',
        label: 'Topaz-3',
        recurso: 'agente',
        estilo: 'mineral',
        paleta: 'quente',
        art: '<path d="M32 4 C 40 16, 54 30, 54 42 a22 18 0 0 1 -44 0 C 10 30, 24 16, 32 4 Z" fill="#f2a93b"></path><path d="M32 4 C 40 16, 54 30, 54 42 a22 18 0 0 1 -22 18 Z" fill="#c77d12" opacity=".45"></path><path d="M32 22 L44 40 L32 52 L20 40 Z" fill="#f2a93b" stroke="#fff" stroke-opacity=".5" stroke-width="1"></path><path d="M32 4 V22M54 42 L44 40M10 42 L20 40M32 60 V52" stroke="#fff" stroke-opacity=".35" stroke-width="1.2"></path><circle cx="28" cy="38" r="2.4" fill="#fff"></circle><circle cx="28" cy="38" r="1.2" fill="#1c1c21"></circle><circle cx="36" cy="38" r="2.4" fill="#fff"></circle><circle cx="36" cy="38" r="1.2" fill="#1c1c21"></circle>',
      },
      {
        id: 'ametis',
        label: 'Ametis',
        recurso: 'agente',
        estilo: 'mineral',
        paleta: 'fria',
        art: '<path d="M4 46 C 4 22, 20 8, 32 8 C 46 8, 60 22, 60 46 Z" fill="#8a7561"></path><path d="M10 46 C 10 26, 22 14, 32 14 C 44 14, 54 26, 54 46 Z" fill="#e9e2d3"></path><polygon points="14,46 19,34 24,46" fill="#6c38b8"></polygon><polygon points="20,46 25,26 30,46" fill="#9b5de5"></polygon><polygon points="27,46 32,22 37,46" fill="#6c38b8"></polygon><polygon points="34,46 39,28 44,46" fill="#9b5de5"></polygon><polygon points="40,46 45,32 50,46" fill="#6c38b8"></polygon><path d="M4 46 H60 C 60 54, 50 58, 32 58 C 14 58, 4 54, 4 46 Z" fill="#6c5a49"></path><circle cx="26" cy="52" r="2.4" fill="#fff"></circle><circle cx="26" cy="52" r="1.2" fill="#1c1c21"></circle><circle cx="38" cy="52" r="2.4" fill="#fff"></circle><circle cx="38" cy="52" r="1.2" fill="#1c1c21"></circle>',
      },
      {
        id: 'nacar',
        label: 'Nácar',
        recurso: 'agente',
        estilo: 'mineral',
        paleta: 'neutra',
        art: '<path d="M6 52 C 14 40, 50 40, 58 52 C 50 60, 14 60, 6 52 Z" fill="#e7dccd"></path><circle cx="32" cy="32" r="20" fill="#f4efe7"></circle><circle cx="32" cy="32" r="20" fill="#e0609a" opacity=".12"></circle><ellipse cx="25" cy="24" rx="6" ry="4" fill="#fff" transform="rotate(-30 25 24)"></ellipse><path d="M14 40 A20 20 0 0 0 46 46" stroke="#c9bfb0" stroke-width="2" fill="none"></path><circle cx="26" cy="34" r="4.2" fill="#fff"></circle><circle cx="26" cy="34" r="2.1" fill="#1c1c21"></circle><circle cx="38" cy="34" r="4.2" fill="#fff"></circle><circle cx="38" cy="34" r="2.1" fill="#1c1c21"></circle><path d="M27 42 Q32 47 37 42" fill="none" stroke="#1c1c21" stroke-width="2" stroke-linecap="round"></path>',
      },
      {
        id: 'obsidia',
        label: 'Obsídia',
        recurso: 'agente',
        estilo: 'mineral',
        paleta: 'neutra',
        art: '<polygon points="30,4 50,18 58,44 36,60 12,52 6,26" fill="#1d1b26" stroke="#6b6688" stroke-width="2" stroke-linejoin="round"></polygon><polygon points="30,4 50,18 34,30 6,26" fill="#3a3650"></polygon><polygon points="34,30 50,18 58,44" fill="#2a2738"></polygon><path d="M14 22 L26 12" stroke="#9aa6ff" stroke-width="2" stroke-linecap="round" opacity=".7"></path><circle cx="28" cy="40" r="3" fill="#fff"></circle><circle cx="28" cy="40" r="1.5" fill="#1c1c21"></circle><circle cx="40" cy="40" r="3" fill="#fff"></circle><circle cx="40" cy="40" r="1.5" fill="#1c1c21"></circle>',
      },
      {
        id: 'pyra',
        label: 'Pyra',
        recurso: 'agente',
        estilo: 'mineral',
        paleta: 'quente',
        art: '<polygon points="6,34 18,28.0 30,34 18,40.0" fill="#f6d77a"></polygon><polygon points="6,34 18,40.0 18,52.0 6,46" fill="#e3b341"></polygon><polygon points="30,34 18,40.0 18,52.0 30,46" fill="#a77c14"></polygon><polygon points="30,38 40,33.0 50,38 40,43.0" fill="#f6d77a"></polygon><polygon points="30,38 40,43.0 40,53.0 30,48" fill="#e3b341"></polygon><polygon points="50,38 40,43.0 40,53.0 50,48" fill="#a77c14"></polygon><polygon points="18,14 32,7.0 46,14 32,21.0" fill="#f6d77a"></polygon><polygon points="18,14 32,21.0 32,35.0 18,28" fill="#e3b341"></polygon><polygon points="46,14 32,21.0 32,35.0 46,28" fill="#a77c14"></polygon><circle cx="27" cy="30" r="2.4" fill="#fff"></circle><circle cx="27" cy="30" r="1.2" fill="#1c1c21"></circle><circle cx="37" cy="30" r="2.4" fill="#fff"></circle><circle cx="37" cy="30" r="1.2" fill="#1c1c21"></circle>',
      },
      {
        id: 'turmal',
        label: 'Turmal',
        recurso: 'agente',
        estilo: 'mineral',
        paleta: 'quente',
        art: '<polygon points="22,10 32,4 42,10 42,34 22,34" fill="#e0609a"></polygon><polygon points="22,34 42,34 42,54 32,60 22,54" fill="#1fa37a"></polygon><polygon points="32,4 42,10 42,54 32,60" fill="#000" opacity=".15"></polygon><path d="M32 4 V60" stroke="#fff" stroke-opacity=".4" stroke-width="1.2"></path><path d="M22 34 H42" stroke="#fff" stroke-opacity=".6" stroke-width="1.5"></path><circle cx="27" cy="22" r="2.4" fill="#fff"></circle><circle cx="27" cy="22" r="1.2" fill="#1c1c21"></circle><circle cx="37" cy="22" r="2.4" fill="#fff"></circle><circle cx="37" cy="22" r="1.2" fill="#1c1c21"></circle>',
      },
      {
        id: 'opalina',
        label: 'Opalina',
        recurso: 'agente',
        estilo: 'mineral',
        paleta: 'fria',
        art: '<ellipse cx="32" cy="32" rx="26" ry="22" fill="#eef3f8"></ellipse><ellipse cx="22" cy="24" rx="6" ry="3.5999999999999996" fill="#7ef0d4" opacity=".75" transform="rotate(20 22 24)"></ellipse><ellipse cx="40" cy="22" rx="5" ry="3.0" fill="#ffb4d1" opacity=".75" transform="rotate(-30 40 22)"></ellipse><ellipse cx="44" cy="38" rx="6" ry="3.5999999999999996" fill="#9ab8ff" opacity=".75" transform="rotate(40 44 38)"></ellipse><ellipse cx="24" cy="42" rx="5" ry="3.0" fill="#ffe08a" opacity=".75" transform="rotate(-10 24 42)"></ellipse><ellipse cx="33" cy="32" rx="4" ry="2.4" fill="#c9a7ff" opacity=".75" transform="rotate(70 33 32)"></ellipse><ellipse cx="32" cy="32" rx="26" ry="22" fill="none" stroke="#c7d2de" stroke-width="2"></ellipse><ellipse cx="22" cy="18" rx="7" ry="3" fill="#fff" opacity=".8"></ellipse><circle cx="27" cy="34" r="2.6" fill="#fff"></circle><circle cx="27" cy="34" r="1.3" fill="#1c1c21"></circle><circle cx="37" cy="34" r="2.6" fill="#fff"></circle><circle cx="37" cy="34" r="1.3" fill="#1c1c21"></circle>',
      },
      {
        id: 'lazuli',
        label: 'Lazuli',
        recurso: 'agente',
        estilo: 'mineral',
        paleta: 'fria',
        art: '<path d="M10 28 C 8 14, 24 6, 38 8 C 52 10, 60 22, 56 38 C 52 54, 38 60, 24 56 C 12 52, 12 40, 10 28 Z" fill="#1c3e9e"></path><path d="M14 26 C 14 16, 26 10, 36 12 C 26 16, 20 24, 22 36 C 18 34, 15 30, 14 26 Z" fill="#2f5fd8"></path><circle cx="18" cy="24" r="1.6" fill="#e3b341"></circle><circle cx="44" cy="20" r="1.2" fill="#e3b341"></circle><circle cx="48" cy="40" r="1.8" fill="#e3b341"></circle><circle cx="22" cy="46" r="1.3" fill="#e3b341"></circle><circle cx="36" cy="50" r="1" fill="#e3b341"></circle><circle cx="28" cy="32" r="3" fill="#fff"></circle><circle cx="28" cy="32" r="1.5" fill="#1c1c21"></circle><circle cx="40" cy="32" r="3" fill="#fff"></circle><circle cx="40" cy="32" r="1.5" fill="#1c1c21"></circle>',
      },
      {
        id: 'aurea',
        label: 'Áurea',
        recurso: 'equipe',
        estilo: 'mineral',
        paleta: 'quente',
        art: '<ellipse cx="32" cy="42" rx="20" ry="16" fill="none" stroke="#e3b341" stroke-width="6"></ellipse><ellipse cx="32" cy="42" rx="20" ry="16" fill="none" stroke="#a77c14" stroke-width="1.5" stroke-dasharray="0 0" opacity=".5"></ellipse><path d="M22 26 L26 18 H38 L42 26 Z" fill="#a77c14"></path><g transform="translate(16,0) scale(.5)"><polygon points="57.9,42.7 42.7,57.9 21.3,57.9 6.1,42.7 6.1,21.3 21.3,6.1 42.7,6.1 57.9,21.3" fill="#d8344f"></polygon><polygon points="57.9,42.7 42.7,57.9 37.4,44.9 44.9,37.4" fill="#fff" opacity=".18"></polygon><polygon points="42.7,57.9 21.3,57.9 26.6,44.9 37.4,44.9" fill="#a11d36" opacity=".55"></polygon><polygon points="21.3,57.9 6.1,42.7 19.1,37.4 26.6,44.9" fill="#fff" opacity=".18"></polygon><polygon points="6.1,42.7 6.1,21.3 19.1,26.6 19.1,37.4" fill="#a11d36" opacity=".55"></polygon><polygon points="6.1,21.3 21.3,6.1 26.6,19.1 19.1,26.6" fill="#fff" opacity=".18"></polygon><polygon points="21.3,6.1 42.7,6.1 37.4,19.1 26.6,19.1" fill="#a11d36" opacity=".55"></polygon><polygon points="42.7,6.1 57.9,21.3 44.9,26.6 37.4,19.1" fill="#fff" opacity=".18"></polygon><polygon points="57.9,21.3 57.9,42.7 44.9,37.4 44.9,26.6" fill="#a11d36" opacity=".55"></polygon><polygon points="44.9,37.4 37.4,44.9 26.6,44.9 19.1,37.4 19.1,26.6 26.6,19.1 37.4,19.1 44.9,26.6" fill="#d8344f" stroke="#fff" stroke-opacity=".5" stroke-width="1"></polygon><path d="M20 22 L25 26" stroke="#fff" stroke-width="2.5" stroke-linecap="round" opacity=".8"></path></g>',
      },
    ],
  },
]

export function findIdentityConcept(familyId: string, conceptId: string): IdentityConcept {
  const concept = IDENTITY_FAMILIES.find((family) => family.id === familyId)?.concepts.find(
    (item) => item.id === conceptId,
  )
  if (!concept) throw new Error(`Conceito de identidade não encontrado: ${familyId}/${conceptId}`)
  return concept
}
