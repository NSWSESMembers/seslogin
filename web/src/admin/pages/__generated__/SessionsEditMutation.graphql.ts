/**
 * @generated SignedSource<<62bc7d344d2e8d6fe240ee64c08a9084>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SessionsEditMutation$variables = {
  config?: string | null | undefined;
  healthcheckUrl?: string | null | undefined;
  id: string;
  name: string;
};
export type SessionsEditMutation$data = {
  readonly updateSession: {
    readonly config: any;
    readonly healthcheckUrl: string | null | undefined;
    readonly id: string;
    readonly name: string;
  };
};
export type SessionsEditMutation = {
  response: SessionsEditMutation$data;
  variables: SessionsEditMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "config"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "healthcheckUrl"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "id"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "name"
},
v4 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "config",
        "variableName": "config"
      },
      {
        "kind": "Variable",
        "name": "healthcheckUrl",
        "variableName": "healthcheckUrl"
      },
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "id"
      },
      {
        "kind": "Variable",
        "name": "name",
        "variableName": "name"
      }
    ],
    "concreteType": "Session",
    "kind": "LinkedField",
    "name": "updateSession",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "id",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "name",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "config",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "healthcheckUrl",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*: any*/),
      (v1/*: any*/),
      (v2/*: any*/),
      (v3/*: any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "SessionsEditMutation",
    "selections": (v4/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v2/*: any*/),
      (v3/*: any*/),
      (v0/*: any*/),
      (v1/*: any*/)
    ],
    "kind": "Operation",
    "name": "SessionsEditMutation",
    "selections": (v4/*: any*/)
  },
  "params": {
    "cacheID": "5f7ef9c36dd29cdbbed6b0187f97a0ff",
    "id": null,
    "metadata": {},
    "name": "SessionsEditMutation",
    "operationKind": "mutation",
    "text": "mutation SessionsEditMutation(\n  $id: ID!\n  $name: String!\n  $config: String\n  $healthcheckUrl: String\n) {\n  updateSession(id: $id, name: $name, config: $config, healthcheckUrl: $healthcheckUrl) {\n    id\n    name\n    config\n    healthcheckUrl\n  }\n}\n"
  }
};
})();

(node as any).hash = "e6eb77940e556aade487853850c91e3b";

export default node;
