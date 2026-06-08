# baseUrl is injected by the runner script.

GET {{baseUrl}}/
HTTP 200

GET {{baseUrl}}/rooms
HTTP 200

GET {{baseUrl}}/rooms?$top=2&$skip=1
HTTP 200

GET {{baseUrl}}/rooms?$select=id
HTTP 200

GET {{baseUrl}}/rooms/oak-204
HTTP 200

GET {{baseUrl}}/rooms/nowhere
HTTP 404

GET {{baseUrl}}/rooms/oak-204/printers
HTTP 200

GET {{baseUrl}}/rooms/redw-1002/printers/prn1002-100
HTTP 200

GET {{baseUrl}}/rooms/redw-1002/printers/prn1002-100?$select=id
HTTP 200

POST {{baseUrl}}/rooms
Content-Type: application/json
{
  "id": "sta-204",
  "name": "Studio A 204"
}
HTTP 201

GET {{baseUrl}}/rooms/sta-204
HTTP 200

GET {{baseUrl}}/rooms/sta-204/printers
HTTP 200

POST {{baseUrl}}/rooms/sta-204/printers
Content-Type: application/json
{
  "id": "prn0204-200",
  "model": "Xerox 2100"
}
HTTP 201

GET {{baseUrl}}/rooms
HTTP 200