# Stylet et tablettes : état par plateforme

Ce document dit ce que Calame obtient réellement des stylets, plateforme par plateforme, et ce qui
manque. Il distingue ce qui a été **vérifié** de ce qui est seulement **attendu**.

## Ce qui est utilisé aujourd'hui

Calame reçoit les entrées via `winit` et `egui`. Pour un stylet, les événements arrivent comme des
événements tactiles qui portent une **force** (`Touch::force`), normalisée entre 0 et 1 : c'est la
pression. La souris n'en fournit pas ; on prend alors 0,5.

La pression règle l'épaisseur du stylo (de 25 % à 100 % de l'épaisseur choisie). Le surligneur a une
épaisseur constante.

| Information | Windows | Linux | macOS |
|---|---|---|---|
| Position | Vérifié | Attendu | Attendu |
| Pression | Vérifié sur la machine de développement | Non vérifié | Non vérifié |
| Inclinaison | Non utilisée (toujours absente) | Non utilisée | Non utilisée |
| Bouton du stylet | Non géré | Non géré | Non géré |
| Extrémité gomme | Non gérée | Non gérée | Non gérée |
| Rejet de la paume | Non géré | Non géré | Non géré |

« Vérifié » signifie qu'un utilisateur a testé la pression avec un stylet sous Windows 11 et que
l'épaisseur suit bien la pression. Les modèles de stylet et de tablette testés ne sont pas recensés :
si vous essayez Calame avec une Wacom, une Surface Pen ou autre, dites-nous ce qui marche.

## Ce qui manque et pourquoi

`winit` n'expose pas de façon fiable le bouton du stylet, l'extrémité gomme, l'inclinaison ni le
type de pointeur (stylet ou doigt) selon les plateformes. Sans le type de pointeur, on ne peut pas
écarter la paume : un contact tactile sans force est indiscernable d'un stylet.

La solution prévue est une couche d'accès natif dans la crate `calame-input`, qui produit des
`PointerSample` (position, pression, inclinaison, bouton, gomme, type) indépendants de la plateforme :

- **Windows** : événements `WM_POINTER` (Windows Ink), via un sous-classement de la fenêtre. Ils
  donnent le type de pointeur, la pression, l'inclinaison, les boutons et la gomme ;
- **Linux** : XInput2 sur X11, protocole `tablet-v2` sur Wayland (ou libinput) ;
- **macOS** : `NSEvent` (pression, inclinaison, sous-type tablette, gomme).

Cette couche n'est pas écrite. `calame-input` ne contient pour l'instant que les types communs.

## Latence

Le trait en cours est redessiné à chaque image. La latence perçue dépend donc de la fréquence
d'affichage et de la synchronisation verticale. `CALAME_VSYNC=0` désactive la synchronisation
verticale pour réduire la latence, au prix possible d'un déchirement de l'image.
