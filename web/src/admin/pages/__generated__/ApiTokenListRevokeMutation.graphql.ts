/**
 * @generated SignedSource<<64481caaab256cb224d052f9b068b659>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ApiTokenListRevokeMutation$variables = {
  id: string;
};
export type ApiTokenListRevokeMutation$data = {
  readonly revokeApiToken: boolean;
};
export type ApiTokenListRevokeMutation = {
  response: ApiTokenListRevokeMutation$data;
  variables: ApiTokenListRevokeMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "id"
  }
],
v1 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "id"
      }
    ],
    "kind": "ScalarField",
    "name": "revokeApiToken",
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "ApiTokenListRevokeMutation",
    "selections": (v1/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "ApiTokenListRevokeMutation",
    "selections": (v1/*:: as any*/)
  },
  "params": {
    "cacheID": "5e92220fbfdb6557b49be6e32d4a8e32",
    "id": null,
    "metadata": {},
    "name": "ApiTokenListRevokeMutation",
    "operationKind": "mutation",
    "text": "mutation ApiTokenListRevokeMutation(\n  $id: ID!\n) {\n  revokeApiToken(id: $id)\n}\n"
  }
};
})();

(node as any).hash = "0b9eb11b66d6f6fb4142e339c43f684b";

export default node;
