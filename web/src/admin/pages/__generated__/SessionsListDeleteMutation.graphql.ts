/**
 * @generated SignedSource<<b80e12cd91c5ab0945281b50b7a34b67>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SessionsListDeleteMutation$variables = {
  id: string;
};
export type SessionsListDeleteMutation$data = {
  readonly deleteSession: boolean;
};
export type SessionsListDeleteMutation = {
  response: SessionsListDeleteMutation$data;
  variables: SessionsListDeleteMutation$variables;
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
    "name": "deleteSession",
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "SessionsListDeleteMutation",
    "selections": (v1/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "SessionsListDeleteMutation",
    "selections": (v1/*:: as any*/)
  },
  "params": {
    "cacheID": "6883fa8e84a338957abb35a67b4a81a0",
    "id": null,
    "metadata": {},
    "name": "SessionsListDeleteMutation",
    "operationKind": "mutation",
    "text": "mutation SessionsListDeleteMutation(\n  $id: ID!\n) {\n  deleteSession(id: $id)\n}\n"
  }
};
})();

(node as any).hash = "5696ca21ce1286b7994fba16e03148bd";

export default node;
