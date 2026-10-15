/**
 * @generated SignedSource<<6c2130d9071a2f5eee42ab01be2f8420>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SettingsConnectedAppsRevokeMutation$variables = {
  id: string;
};
export type SettingsConnectedAppsRevokeMutation$data = {
  readonly revokeOauthGrant: boolean;
};
export type SettingsConnectedAppsRevokeMutation = {
  response: SettingsConnectedAppsRevokeMutation$data;
  variables: SettingsConnectedAppsRevokeMutation$variables;
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
    "name": "revokeOauthGrant",
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "SettingsConnectedAppsRevokeMutation",
    "selections": (v1/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "SettingsConnectedAppsRevokeMutation",
    "selections": (v1/*:: as any*/)
  },
  "params": {
    "cacheID": "703341df4a3bbd6bd83b77567cb46950",
    "id": null,
    "metadata": {},
    "name": "SettingsConnectedAppsRevokeMutation",
    "operationKind": "mutation",
    "text": "mutation SettingsConnectedAppsRevokeMutation(\n  $id: ID!\n) {\n  revokeOauthGrant(id: $id)\n}\n"
  }
};
})();

(node as any).hash = "19924be2e95afa73b3c6fd77b06d6303";

export default node;
