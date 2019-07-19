$('form').submit(e => {
  e.preventDefault()
  const asset_id = $(e.target).find('[name=asset_id]').val()

  let contract = $(e.target).find('[name=contract_json]').val()
  try { contract = JSON.parse(contract) }
  catch (e) { return showError('Invalid contract JSON: '+e) }

  fetch(`https://assets.blockstream.info/`, {
    method: 'POST'
  , mode: 'cors'
  , headers: { 'Content-Type': 'application/json' }
  , body: JSON.stringify({ asset_id, contract })
  })
  .then(r => r.ok ?  r.json()
                  : r.text().then(err => Promise.reject(err)))
  .then(asset => {
    $('#res')
      .removeClass('d-none alert-danger')
      .addClass('alert-success')
      .text('Asset created:')
      .append($('<pre class="mt-2">').text(JSON.stringify(asset, null, 2)))
  })
  .catch(showError)
})

function showError(msg) {
  $('#res')
    .removeClass('d-none alert-success')
    .addClass('alert-danger')
    .text(msg)
}
