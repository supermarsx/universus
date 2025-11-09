export function createShipyardDOM(): void {
  document.body.innerHTML = `
    <select id="shipyardLocationSelect"></select>
    <div id="shipyardLocationStatus"></div>
    <div id="shipyardLocationResources" class="hidden"></div>
    <div id="shipsGrid"></div>
    <div id="defenseGrid"></div>

    <div id="shipProductionQueue" style="display:none">
      <h3></h3>
      <div id="shipQueue"></div>
    </div>
  `;
}
